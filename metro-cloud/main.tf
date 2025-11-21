# Using Google's Terraform template: https://github.com/terraform-google-modules/terraform-docs-samples/blob/main/functions/basic/main.tf

terraform {
  required_providers {
    google = {
      source  = "hashicorp/google"
      version = ">= 4.34.0"
    }
  }
}

resource "google_service_account" "worker_sa" {
  account_id   = "remetro-worker-sa"
  display_name = "reMetro Worker SA"
}

resource "random_id" "default" {
  byte_length = 8
}

resource "google_storage_bucket" "default" {
  name                        = "${random_id.default.hex}-gcf-source"
  location                    = "US"
  uniform_bucket_level_access = true
}

data "archive_file" "default" {
  type        = "zip"
  output_path = "/tmp/function-source.zip"
  source_dir  = "../metro-fetch/"
}

resource "google_storage_bucket_object" "object" {
  name   = "function-source.zip"
  bucket = google_storage_bucket.default.name
  source = data.archive_file.default.output_path # Add path to the zipped function source code
}

resource "google_cloudfunctions2_function" "fetch" {
  name        = "fetch"
  location    = var.gcp_region
  description = "Fetches data from the official WMATA API."

  build_config {
    runtime     = "nodejs22"
    entry_point = "tick"
    source {
      storage_source {
        bucket = google_storage_bucket.default.name
        object = google_storage_bucket_object.object.name
      }
    }
  }

  service_config {
    max_instance_count = 1
    available_memory   = "128M"
    # Data will be stale by this point
    timeout_seconds    = 10
    service_account_email = google_service_account.worker_sa.email
    environment_variables = {
      TASKS_QUEUE  = google_cloud_tasks_queue.loop.name
      TASKS_REGION = var.gcp_region
    }
  }
}

resource "google_cloud_run_service_iam_member" "tick_invoker" {
  location = var.gcp_region
  project  = var.gcp_project
  service  = google_cloudfunctions2_function.fetch.service_config[0].service
  role     = "roles/run.invoker"
  member   = "serviceAccount:${google_service_account.worker_sa.email}"
}

resource "google_cloud_run_service_iam_member" "member" {
  location = google_cloudfunctions2_function.fetch.location
  service  = google_cloudfunctions2_function.fetch.name
  role     = "roles/run.invoker"
  member   = "allUsers"
}

output "function_uri" {
  value = google_cloudfunctions2_function.fetch.service_config[0].uri
}