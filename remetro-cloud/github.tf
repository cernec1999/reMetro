# If you don't already have a provider in the account, create it instead:
resource "aws_iam_openid_connect_provider" "github" {
  tags            = local.tags
  url             = "https://token.actions.githubusercontent.com"
  client_id_list  = ["sts.amazonaws.com"]
}

data "aws_iam_policy_document" "github_actions_assume_role" {
  statement {
    effect = "Allow"
    actions = ["sts:AssumeRoleWithWebIdentity"]

    principals {
      type        = "Federated"
      identifiers = [aws_iam_openid_connect_provider.github.arn]
      # if using resource provider instead, use aws_iam_openid_connect_provider.github.arn
    }

    condition {
      test     = "StringEquals"
      variable = "token.actions.githubusercontent.com:aud"
      values   = ["sts.amazonaws.com"]
    }

    # Restrict to your repo + main branch.
    condition {
      test     = "StringLike"
      variable = "token.actions.githubusercontent.com:sub"
      values   = [
        "repo:cernec1999/reMetro:ref:refs/heads/main"
      ]
    }
  }
}

resource "aws_iam_role" "github_actions" {
  tags               = local.tags
  name               = "remetro-github-actions-role"
  assume_role_policy = data.aws_iam_policy_document.github_actions_assume_role.json
}

data "aws_caller_identity" "current" {}

# Minimal policy for:
# - pushing images to ECR
# - updating ECS task defs/services
# - reading IAM roles so TF can pass them
# - managing the specific infra in this stack
data "aws_iam_policy_document" "github_actions_policy" {
  statement {
    sid     = "EcrAuthToken"
    effect  = "Allow"
    actions = ["ecr:GetAuthorizationToken"]
    resources = ["*"]
  }

  # --------------------------
  # ECR: push/pull ONLY this repo
  # --------------------------
  statement {
    sid    = "EcrPushPullRepo"
    effect = "Allow"
    actions = [
      "ecr:BatchCheckLayerAvailability",
      "ecr:GetDownloadUrlForLayer",
      "ecr:BatchGetImage",
      "ecr:InitiateLayerUpload",
      "ecr:UploadLayerPart",
      "ecr:CompleteLayerUpload",
      "ecr:PutImage",
      "ecr:DescribeImages",
      "ecr:ListImages",
      "ecr:TagResource"
    ]
    resources = [aws_ecr_repository.remetro_fetch.arn]
  }

  # Allow describing the repo itself
  statement {
    sid     = "EcrDescribeRepo"
    effect  = "Allow"
    actions = ["ecr:DescribeRepositories"]
    resources = [aws_ecr_repository.remetro_fetch.arn]
  }

  # --------------------------
  # ECS: update service ONLY for this cluster/service
  # --------------------------
  statement {
    sid    = "EcsServiceDeploy"
    effect = "Allow"
    actions = [
      "ecs:UpdateService",
      "ecs:DescribeServices"
    ]
    resources = [
      aws_ecs_service.remetro_fetch.arn,
      aws_ecs_cluster.remetro.arn
    ]
  }

  # RegisterTaskDefinition does NOT support resource-level permissions
  statement {
    sid     = "EcsRegisterTaskDef"
    effect  = "Allow"
    actions = ["ecs:RegisterTaskDefinition"]
    resources = ["*"]
  }

  # Describe specific family revisions only
  statement {
    sid     = "EcsDescribeTaskDefFamily"
    effect  = "Allow"
    actions = ["ecs:DescribeTaskDefinition"]
    resources = [
      "arn:aws:ecs:${var.aws_region}:${data.aws_caller_identity.current.account_id}:task-definition/remetro-fetch*"
    ]
  }

  # List/describe tasks, but only inside this cluster
  statement {
    sid     = "EcsTasksInClusterOnly"
    effect  = "Allow"
    actions = [
      "ecs:ListTasks",
      "ecs:DescribeTasks"
    ]
    resources = ["*"] # tasks are dynamic ARNs
    condition {
      test     = "ArnEquals"
      variable = "ecs:cluster"
      values   = [aws_ecs_cluster.remetro.arn]
    }
  }

  # --------------------------
  # IAM: only pass the two ECS roles created by TF
  # --------------------------
  statement {
    sid     = "AllowPassOnlyEcsRoles"
    effect  = "Allow"
    actions = ["iam:PassRole"]
    resources = [
      aws_iam_role.ecs_task_execution.arn,
      aws_iam_role.ecs_task.arn
    ]
  }

  # --------------------------
  # CloudWatch logs: only your log group
  # CreateLogGroup doesn't support ARNs well when creating -> tag-gated below
  # --------------------------
  statement {
    sid     = "LogsManageRemetroGroup"
    effect  = "Allow"
    actions = [
      "logs:PutRetentionPolicy",
      "logs:DescribeLogGroups"
    ]
    resources = [
      aws_cloudwatch_log_group.remetro_fetch.arn
    ]
  }

  # --------------------------
  # Terraform create/update/destroy for tagged resources
  # --------------------------
  statement {
    sid    = "TerraformTaggedResources"
    effect = "Allow"
    actions = [
      # ECR
      "ecr:CreateRepository",
      "ecr:DeleteRepository",
      "ecr:PutLifecyclePolicy",
      "ecr:DeleteLifecyclePolicy",

      # ECS
      "ecs:CreateCluster",
      "ecs:DeleteCluster",
      "ecs:CreateService",
      "ecs:DeleteService",
      "ecs:DeregisterTaskDefinition",

      # EC2 networking
      "ec2:CreateSecurityGroup",
      "ec2:DeleteSecurityGroup",
      "ec2:AuthorizeSecurityGroupIngress",
      "ec2:RevokeSecurityGroupIngress",
      "ec2:AuthorizeSecurityGroupEgress",
      "ec2:RevokeSecurityGroupEgress",
      "ec2:DescribeVpcs",
      "ec2:DescribeSubnets",
      "ec2:DescribeSecurityGroups",

      # IAM resources TF manages
      "iam:CreateRole",
      "iam:DeleteRole",
      "iam:AttachRolePolicy",
      "iam:DetachRolePolicy",
      "iam:PutRolePolicy",
      "iam:DeleteRolePolicy",
      "iam:GetRole",
      "iam:ListRolePolicies",
      "iam:ListAttachedRolePolicies",

      # Logs
      "logs:CreateLogGroup",
      "logs:DeleteLogGroup"
    ]
    resources = ["*"]

    # Only allow if TF is creating/updating resources in the remetro project
    condition {
      test     = "StringEquals"
      variable = "aws:RequestTag/Project"
      values   = ["remetro"]
    }
    condition {
      test     = "StringEquals"
      variable = "aws:ResourceTag/Project"
      values   = ["remetro"]
    }
  }
}

resource "aws_iam_policy" "github_actions" {
  tags = local.tags
  name   = "remetro-github-actions-policy"
  policy = data.aws_iam_policy_document.github_actions_policy.json
}

resource "aws_iam_role_policy_attachment" "github_actions" {
  role       = aws_iam_role.github_actions.name
  policy_arn = aws_iam_policy.github_actions.arn
}

output "github_actions_role_arn" {
  value = aws_iam_role.github_actions.arn
}
