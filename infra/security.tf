# ---------------------------------------------------------------------------
# Security group — SSH from user IP, bench port between instances
# ---------------------------------------------------------------------------

resource "aws_security_group" "bench" {
  name        = "harrow-bench"
  description = "Harrow benchmark: SSH + inter-instance bench traffic"

  tags = merge(local.common_tags, { Name = "harrow-bench" })
}

# SSH from caller's public IP
resource "aws_vpc_security_group_ingress_rule" "ssh" {
  security_group_id = aws_security_group.bench.id
  description       = "SSH from deployer IP"
  ip_protocol       = "tcp"
  from_port         = 22
  to_port           = 22
  cidr_ipv4         = local.my_ip
}

# Bench port (3000) between instances in the same SG
resource "aws_vpc_security_group_ingress_rule" "bench_port" {
  security_group_id            = aws_security_group.bench.id
  description                  = "Bench port between instances"
  ip_protocol                  = "tcp"
  from_port                    = 3000
  to_port                      = 3100
  referenced_security_group_id = aws_security_group.bench.id
}

# All outbound
resource "aws_vpc_security_group_egress_rule" "all_out" {
  security_group_id = aws_security_group.bench.id
  ip_protocol       = "-1"
  cidr_ipv4         = "0.0.0.0/0"
}
