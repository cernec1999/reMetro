variable "aws_region" {
  description = "The AWS region to deploy resources in."
  type        = string
}

variable "ecr_repo_name" {
  description = "The name of the ECR repository for the remetro-fetch container."
  type        = string
  default     = "remetro-fetch"
}

variable "task_cpu" {
  description = "The CPU units for the ECS task. 512 gives the JVM a decent startup budget."
  type        = number
  default     = 512
}

variable "task_memory" {
  description = "The memory (in MiB) for the ECS task. Spring Boot JVM needs at least 512 MiB; 1024 is comfortable."
  type        = number
  default     = 1024
}

variable "image_tag" {
  description = "The tag of the Docker image to deploy."
  type        = string
  default     = "latest"
}

variable "wmata_api_timeout" {
  description = "The timeout duration (in seconds) for WMATA API requests."
  type        = number
  default     = 20
}

variable "mqtt_broker" {
  description = "The MQTT broker address."
  type        = string
}

variable "mqtt_port" {
  description = "The MQTT broker port."
  type        = number
  default     = 1883
}

variable "mqtt_client_id" {
  description = "The MQTT client ID."
  type        = string
  default     = "reMetroClient"
}