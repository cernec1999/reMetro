# The GitHub Actions OIDC provider must be bootstrapped once by an admin before
# any CI run can authenticate. We look it up here rather than creating it so we
# don't hit a circular dependency (GitHub Actions needs the provider to assume a
# role, but can't create the provider without already being authenticated).
data "aws_iam_openid_connect_provider" "github" {
  url = "https://token.actions.githubusercontent.com"
}

data "aws_iam_policy_document" "github_actions_assume_role" {
  statement {
    effect  = "Allow"
    actions = ["sts:AssumeRoleWithWebIdentity"]

    principals {
      type        = "Federated"
      identifiers = [data.aws_iam_openid_connect_provider.github.arn]
    }

    condition {
      test     = "StringEquals"
      variable = "token.actions.githubusercontent.com:aud"
      values   = ["sts.amazonaws.com"]
    }

    # Restrict to main branch pushes only
    condition {
      test     = "StringLike"
      variable = "token.actions.githubusercontent.com:sub"
      values   = ["repo:cernec1999/reMetro:ref:refs/heads/main"]
    }
  }
}

resource "aws_iam_role" "github_actions" {
  tags               = local.tags
  name               = "remetro-github-actions-role"
  assume_role_policy = data.aws_iam_policy_document.github_actions_assume_role.json
}

data "aws_caller_identity" "current" {}

data "aws_iam_policy_document" "github_actions_policy" {

  # --------------------------
  # Read-only: Terraform state refresh + Describe for all managed resources.
  # Read operations never carry aws:RequestTag so they must be unconditional.
  # --------------------------
  statement {
    sid    = "ReadRemetroResources"
    effect = "Allow"
    actions = [
      # IAM
      "iam:GetRole",
      "iam:GetPolicy",
      "iam:GetPolicyVersion",
      "iam:ListPolicyVersions",
      "iam:ListRolePolicies",
      "iam:ListAttachedRolePolicies",
      "iam:ListOpenIDConnectProviders",
      "iam:GetOpenIDConnectProvider",
      # ECR
      "ecr:DescribeRepositories",
      "ecr:ListTagsForResource",
      "ecr:GetLifecyclePolicy",
      # ECS
      "ecs:DescribeClusters",
      "ecs:DescribeServices",
      "ecs:DescribeTaskDefinition",
      "ecs:DescribeCapacityProviders",
      # CloudWatch Logs
      "logs:DescribeLogGroups",
      "logs:ListTagsForResource",
      # IAM inline policies
      "iam:GetRolePolicy",
      # Secrets Manager
      "secretsmanager:DescribeSecret",
      "secretsmanager:GetResourcePolicy",
    ]
    resources = ["*"]
  }

  # --------------------------
  # ECR: authenticate + push/pull this repo
  # --------------------------
  statement {
    sid       = "EcrAuthToken"
    effect    = "Allow"
    actions   = ["ecr:GetAuthorizationToken"]
    resources = ["*"]
  }

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
      "ecr:TagResource",
    ]
    resources = [aws_ecr_repository.remetro_fetch.arn]
  }

  # --------------------------
  # ECS: deploy (update service + register task definitions)
  # --------------------------
  statement {
    sid    = "EcsServiceDeploy"
    effect = "Allow"
    actions = [
      "ecs:UpdateService",
      "ecs:ListTasks",
      "ecs:DescribeTasks",
    ]
    resources = ["*"]
    condition {
      test     = "ArnEquals"
      variable = "ecs:cluster"
      values   = [aws_ecs_cluster.remetro.arn]
    }
  }

  # RegisterTaskDefinition and DescribeTaskDefinition don't support resource-level permissions
  statement {
    sid    = "EcsTaskDef"
    effect = "Allow"
    actions = [
      "ecs:RegisterTaskDefinition",
      "ecs:DeregisterTaskDefinition",
    ]
    resources = ["*"]
  }

  # --------------------------
  # IAM: pass ECS roles
  # --------------------------
  statement {
    sid     = "IamPassEcsRoles"
    effect  = "Allow"
    actions = ["iam:PassRole"]
    resources = [
      aws_iam_role.ecs_task_execution.arn,
      aws_iam_role.ecs_task.arn,
    ]
  }

  # --------------------------
  # Mutating writes — all tag-gated on aws:RequestTag/Project=remetro
  # so Terraform must tag resources it creates.
  # --------------------------
  statement {
    sid    = "IamWriteRemetro"
    effect = "Allow"
    actions = [
      "iam:CreateRole",
      "iam:DeleteRole",
      "iam:TagRole",
      "iam:AttachRolePolicy",
      "iam:DetachRolePolicy",
      "iam:PutRolePolicy",
      "iam:DeleteRolePolicy",
      "iam:CreatePolicy",
      "iam:DeletePolicy",
      "iam:CreatePolicyVersion",
    ]
    resources = ["*"]
    condition {
      test     = "StringEquals"
      variable = "aws:RequestTag/Project"
      values   = ["remetro"]
    }
  }

  statement {
    sid    = "EcsWriteRemetro"
    effect = "Allow"
    actions = [
      "ecs:CreateCluster",
      "ecs:DeleteCluster",
      "ecs:CreateService",
      "ecs:DeleteService",
      "ecs:TagResource",
    ]
    resources = ["*"]
    condition {
      test     = "StringEquals"
      variable = "aws:RequestTag/Project"
      values   = ["remetro"]
    }
  }

  statement {
    sid    = "EcrWriteRemetro"
    effect = "Allow"
    actions = [
      "ecr:CreateRepository",
      "ecr:DeleteRepository",
      "ecr:PutLifecyclePolicy",
      "ecr:DeleteLifecyclePolicy",
    ]
    resources = ["*"]
    condition {
      test     = "StringEquals"
      variable = "aws:RequestTag/Project"
      values   = ["remetro"]
    }
  }

  statement {
    sid    = "LogsWriteRemetro"
    effect = "Allow"
    actions = [
      "logs:CreateLogGroup",
      "logs:DeleteLogGroup",
      "logs:TagResource",
      "logs:PutRetentionPolicy",
    ]
    resources = ["*"]
    condition {
      test     = "StringEquals"
      variable = "aws:RequestTag/Project"
      values   = ["remetro"]
    }
  }

  statement {
    sid    = "SecretsManagerWriteRemetro"
    effect = "Allow"
    actions = [
      "secretsmanager:CreateSecret",
      "secretsmanager:DeleteSecret",
      "secretsmanager:GetSecretValue",
      "secretsmanager:TagResource",
    ]
    resources = [aws_secretsmanager_secret.wmata_api_key.arn]
  }

  # --------------------------
  # EC2 networking
  # --------------------------
  statement {
    sid    = "Ec2WriteSecurityGroups"
    effect = "Allow"
    actions = [
      "ec2:CreateSecurityGroup",
      "ec2:DeleteSecurityGroup",
      "ec2:AuthorizeSecurityGroupIngress",
      "ec2:RevokeSecurityGroupIngress",
      "ec2:AuthorizeSecurityGroupEgress",
      "ec2:RevokeSecurityGroupEgress",
      "ec2:CreateTags",
    ]
    resources = ["*"]
    condition {
      test     = "StringEquals"
      variable = "aws:RequestTag/Project"
      values   = ["remetro"]
    }
  }

  # Describe* never supports tag conditions
  statement {
    sid    = "Ec2DescribeNetworking"
    effect = "Allow"
    actions = [
      "ec2:DescribeVpcs",
      "ec2:DescribeVpcAttribute",
      "ec2:DescribeSubnets",
      "ec2:DescribeSecurityGroups",
      "ec2:DescribeSecurityGroupRules",
    ]
    resources = ["*"]
  }

  # --------------------------
  # S3 + ECS cluster capacity: Terraform remote state + cluster refresh
  # --------------------------
  statement {
    sid    = "TerraformStateS3"
    effect = "Allow"
    actions = [
      "s3:GetObject",
      "s3:PutObject",
      "s3:DeleteObject",
      "s3:ListBucket",
      "s3:GetBucketVersioning",
    ]
    resources = [
      "arn:aws:s3:::remetro-terraform-state",
      "arn:aws:s3:::remetro-terraform-state/*",
    ]
  }

  statement {
    sid    = "EcsClusterCapacityProviders"
    effect = "Allow"
    actions = [
      "ecs:PutClusterCapacityProviders",
    ]
    resources = [aws_ecs_cluster.remetro.arn]
  }
}

resource "aws_iam_policy" "github_actions" {
  tags   = local.tags
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
