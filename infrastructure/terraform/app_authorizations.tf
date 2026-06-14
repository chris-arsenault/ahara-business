resource "aws_dynamodb_table_item" "seed_chris_app_authorization" {
  table_name = local.app_authorizations_table_name
  hash_key   = "username"

  item = jsonencode({
    username    = { S = "chris" }
    email       = { S = "chris@chris-arsenault.net" }
    displayName = { S = "chris" }
    apps = {
      M = {
        "ahara-business-app" = { S = "admin" }
        canonry              = { S = "admin" }
        svap                 = { S = "admin" }
      }
    }
  })
}
