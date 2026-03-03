terraform {
  required_version = ">= 1.5"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
  }
}

provider "aws" {
  region = var.region
}

# ---------------------------------------------------------------------------
# Data sources
# ---------------------------------------------------------------------------

# Amazon Linux 2023 ARM64 AMI via SSM parameter
data "aws_ssm_parameter" "al2023_arm64" {
  name = "/aws/service/al2023/ami-kernel-default/arm64/latest"
}

# Current caller identity (for tagging)
data "aws_caller_identity" "current" {}

# Pick a single AZ for the placement group
data "aws_availability_zones" "available" {
  state = "available"
}

# Caller's public IP for SSH access
data "http" "my_ip" {
  url = "https://checkip.amazonaws.com"
}

locals {
  my_ip = "${trimspace(data.http.my_ip.response_body)}/32"
  az    = data.aws_availability_zones.available.names[0]
  ami   = data.aws_ssm_parameter.al2023_arm64.value

  common_tags = {
    Project   = "harrow-bench"
    ManagedBy = "terraform"
  }
}

# ---------------------------------------------------------------------------
# Placement group — cluster strategy for minimal network jitter
# ---------------------------------------------------------------------------

resource "aws_placement_group" "bench" {
  name     = "harrow-bench"
  strategy = "cluster"

  tags = local.common_tags
}
