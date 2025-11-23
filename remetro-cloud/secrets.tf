resource "aws_secretsmanager_secret" "wmata_api_key" {
  name        = "remetro/wmata_api_key"
  description = "WMATA API key for remetro-fetch"
  tags        = local.tags
}
