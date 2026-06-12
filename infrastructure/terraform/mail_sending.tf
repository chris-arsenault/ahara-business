resource "aws_cloudwatch_event_rule" "send_worker_schedule" {
  name                = "${local.prefix}-send-worker-schedule"
  description         = "Poll queued outbound mail work."
  schedule_expression = "rate(1 minute)"
}

resource "aws_cloudwatch_event_target" "send_worker_schedule" {
  rule      = aws_cloudwatch_event_rule.send_worker_schedule.name
  target_id = "${local.prefix}-send-worker"
  arn       = module.send_worker.function_arn

  input = jsonencode({
    kind = "scheduled"
  })
}

resource "aws_lambda_permission" "send_worker_schedule" {
  statement_id  = "AllowEventBridgeSendWorkerInvoke"
  action        = "lambda:InvokeFunction"
  function_name = module.send_worker.function_name
  principal     = "events.amazonaws.com"
  source_arn    = aws_cloudwatch_event_rule.send_worker_schedule.arn
}
