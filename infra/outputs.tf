output "server_public_ip" {
  description = "Server instance public IP"
  value       = aws_spot_instance_request.server.public_ip
}

output "server_private_ip" {
  description = "Server instance private IP (use from client)"
  value       = aws_spot_instance_request.server.private_ip
}

output "client_public_ip" {
  description = "Client instance public IP"
  value       = aws_spot_instance_request.client.public_ip
}

output "client_private_ip" {
  description = "Client instance private IP (for OTLP endpoint from server)"
  value       = aws_spot_instance_request.client.private_ip
}

output "ssh_server" {
  description = "SSH command for server instance"
  value       = "ssh -i ~/.ssh/${var.key_name}.pem alpine@${aws_spot_instance_request.server.public_ip}"
}

output "ssh_client" {
  description = "SSH command for client instance"
  value       = "ssh -i ~/.ssh/${var.key_name}.pem alpine@${aws_spot_instance_request.client.public_ip}"
}

output "ecr_harrow_perf_server" {
  description = "ECR repo URL for harrow-perf-server"
  value       = module.ecr_harrow_perf_server.ecr-repository-url
}

output "ecr_axum_perf_server" {
  description = "ECR repo URL for axum-perf-server"
  value       = module.ecr_axum_perf_server.ecr-repository-url
}

output "ecr_spinr" {
  description = "ECR repo URL for spinr (load tester)"
  value       = module.ecr_spinr.ecr-repository-url
}

output "ansible_inventory" {
  description = "Ansible inventory (paste to infra/ansible/inventory.ini)"
  value = templatefile("${path.module}/ansible/inventory.tpl", {
    server_ip = aws_spot_instance_request.server.public_ip
    client_ip = aws_spot_instance_request.client.public_ip
    key_name  = var.key_name
  })
}
