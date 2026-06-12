data "aws_ssm_parameter" "db_username" {
  name = "/ahara/db/ahara-business/username"
}

data "aws_ssm_parameter" "db_password" {
  name = "/ahara/db/ahara-business/password"
}

data "aws_ssm_parameter" "db_database" {
  name = "/ahara/db/ahara-business/database"
}
