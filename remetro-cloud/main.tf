terraform {
  required_version = ">= 1.5.0"
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 6.0"
    }
  }

  backend "s3" {
    bucket       = "remetro-terraform-state"
    key          = "remetro-fetch/terraform.tfstate"
    region       = "us-east-1"
    encrypt      = true
    use_lockfile = true
  }
}

locals {
  tags = {
    Project   = "remetro"
    Service   = "remetro-fetch"
    ManagedBy = "terraform"
  }
}

provider "aws" {
  region = var.aws_region
}

# --------------------------
# Networking (default VPC baseline)
# --------------------------
data "aws_vpc" "default" {
  default = true
}

data "aws_subnets" "default" {
  filter {
    name   = "vpc-id"
    values = [data.aws_vpc.default.id]
  }
}

data "aws_subnet" "default" {
  for_each = toset(data.aws_subnets.default.ids)
  id       = each.value
}

locals {
  public_subnet_ids = [
    for s in data.aws_subnet.default :
    s.id if s.map_public_ip_on_launch
  ]
}

resource "aws_security_group" "remetro_fetch" {
  name        = "remetro-fetch-sg"
  description = "Outbound for fetcher + optional inbound 3000"
  vpc_id      = data.aws_vpc.default.id
  tags        = local.tags

  ingress {
    from_port   = 8080
    to_port     = 8080
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }

  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }
}

# --------------------------
# ECR
# --------------------------
resource "aws_ecr_repository" "remetro_fetch" {
  name = var.ecr_repo_name
  tags = local.tags

  image_scanning_configuration {
    scan_on_push = true
  }
}

resource "aws_ecr_lifecycle_policy" "remetro_fetch" {
  repository = aws_ecr_repository.remetro_fetch.name
  policy = jsonencode({
    rules = [{
      rulePriority = 1
      description  = "Keep last 20 images"
      selection = {
        tagStatus   = "any"
        countType   = "imageCountMoreThan"
        countNumber = 20
      }
      action = { type = "expire" }
    }]
  })
}

# --------------------------
# CloudWatch logs
# --------------------------
resource "aws_cloudwatch_log_group" "remetro_fetch" {
  tags              = local.tags
  name              = "/ecs/remetro-fetch"
  retention_in_days = 14
}

# --------------------------
# IAM for ECS task execution + task role
# --------------------------
data "aws_iam_policy_document" "ecs_task_assume_role" {
  statement {
    effect = "Allow"
    principals {
      type        = "Service"
      identifiers = ["ecs-tasks.amazonaws.com"]
    }
    actions = ["sts:AssumeRole"]
  }
}

resource "aws_iam_role" "ecs_task_execution" {
  tags               = local.tags
  name               = "remetro-fetch-exec-role"
  assume_role_policy = data.aws_iam_policy_document.ecs_task_assume_role.json
}

resource "aws_iam_role_policy_attachment" "ecs_task_execution" {
  role       = aws_iam_role.ecs_task_execution.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AmazonECSTaskExecutionRolePolicy"
}

resource "aws_iam_role" "ecs_task" {
  tags               = local.tags
  name               = "remetro-fetch-task-role"
  assume_role_policy = data.aws_iam_policy_document.ecs_task_assume_role.json
}

data "aws_iam_policy_document" "ecs_task_policy" {
  statement {
    effect = "Allow"
    actions = [
      "ssmmessages:CreateControlChannel",
      "ssmmessages:CreateDataChannel",
      "ssmmessages:OpenControlChannel",
      "ssmmessages:OpenDataChannel"
    ]
    resources = ["*"]
  }
}

resource "aws_iam_role_policy" "ecs_task_exec_ssm" {
  name   = "remetro-fetch-ecs-exec-ssm"
  role   = aws_iam_role.ecs_task.id
  policy = data.aws_iam_policy_document.ecs_task_policy.json
}

# --------------------------
# ECS cluster + capacity providers
# --------------------------
resource "aws_ecs_cluster" "remetro" {
  tags = local.tags
  name = "remetro-cluster"
}

resource "aws_ecs_cluster_capacity_providers" "remetro" {
  cluster_name = aws_ecs_cluster.remetro.name

  capacity_providers = ["FARGATE_SPOT", "FARGATE"]

  default_capacity_provider_strategy {
    capacity_provider = "FARGATE_SPOT"
    weight            = 2 # prefer spot
    base              = 0
  }

  default_capacity_provider_strategy {
    capacity_provider = "FARGATE"
    weight            = 1 # fallback
    base              = 0
  }
}

resource "aws_ecs_task_definition" "remetro_fetch" {
  tags                     = local.tags
  family                   = "remetro-fetch"
  requires_compatibilities = ["FARGATE"]
  network_mode             = "awsvpc"
  cpu                      = var.task_cpu
  memory                   = var.task_memory
  execution_role_arn       = aws_iam_role.ecs_task_execution.arn
  task_role_arn            = aws_iam_role.ecs_task.arn

  container_definitions = jsonencode([
    {
      name      = "remetro-fetch"
      image     = "${aws_ecr_repository.remetro_fetch.repository_url}:${var.image_tag}"
      essential = true

      portMappings = [
        {
          containerPort = 8080
          hostPort      = 8080
          protocol      = "tcp"
        }
      ]

      healthCheck = {
        command     = ["CMD-SHELL", "curl -f http://localhost:8080/health || exit 1"]
        interval    = 30
        timeout     = 5
        retries     = 3
        startPeriod = 60
      }

      logConfiguration = {
        logDriver = "awslogs"
        options = {
          awslogs-group         = aws_cloudwatch_log_group.remetro_fetch.name
          awslogs-region        = var.aws_region
          awslogs-stream-prefix = "ecs"
        }
      }

      # Allow 2 minutes for graceful stop on Spot interruption
      stopTimeout = 120

      secrets = [
        {
          name      = "REMETRO_WMATA_API_KEY"
          valueFrom = aws_secretsmanager_secret.wmata_api_key.arn
        }
      ]

      environment = [
        { name = "REMETRO_WMATA_API_TIMEOUT", value = tostring(var.wmata_api_timeout) },
        { name = "REMETRO_MQTT_BROKER", value = var.mqtt_broker },
        { name = "REMETRO_MQTT_PORT", value = tostring(var.mqtt_port) },
        { name = "REMETRO_MQTT_CLIENT_ID", value = var.mqtt_client_id },
      ]
    }
  ])
}

resource "aws_ecs_service" "remetro_fetch" {
  tags                   = local.tags
  name                   = "remetro-fetch-svc"
  cluster                = aws_ecs_cluster.remetro.id
  task_definition        = aws_ecs_task_definition.remetro_fetch.arn
  desired_count          = 1
  enable_execute_command = true
  force_new_deployment   = true

  # NEW: prefer spot, fallback to on-demand if spot unavailable
  capacity_provider_strategy {
    capacity_provider = "FARGATE_SPOT"
    weight            = 2
    base              = 0
  }

  capacity_provider_strategy {
    capacity_provider = "FARGATE"
    weight            = 1
    base              = 0
  }

  network_configuration {
    subnets          = local.public_subnet_ids
    security_groups  = [aws_security_group.remetro_fetch.id]
    assign_public_ip = true
  }

  # Optional but helps avoid failed deploys leaving you stuck
  deployment_circuit_breaker {
    enable   = true
    rollback = true
  }
}

data "aws_iam_policy_document" "ecs_exec_secrets" {
  statement {
    effect = "Allow"
    actions = [
      "secretsmanager:GetSecretValue",
      "secretsmanager:DescribeSecret"
    ]
    resources = [
      aws_secretsmanager_secret.wmata_api_key.arn
    ]
  }
}

resource "aws_iam_role_policy" "ecs_task_execution_secrets" {
  name   = "remetro-fetch-exec-secrets"
  role   = aws_iam_role.ecs_task_execution.id
  policy = data.aws_iam_policy_document.ecs_exec_secrets.json
}

output "ecr_repository_url" {
  value = aws_ecr_repository.remetro_fetch.repository_url
}
