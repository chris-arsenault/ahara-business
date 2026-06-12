# 0004 - Project-Owned Mail Infrastructure

- Status: Accepted
- Date: 2026-06-09

## Context

The mail foundation owns SES identities, receipt rules, raw MIME storage, SNS topics, and mail-processing Lambdas. The platform provides reusable modules and deployer-role policy bundles, but it does not currently include SES-specific deployer primitives or a general private S3 storage primitive for non-website buckets.

## Decision

Manage mail resources in this project's Terraform, and add first-class SES and mail-storage S3 deployer policy primitives in `ahara-infra`.

## Alternatives considered

- **Own mail resources in `ahara-infra`** - Centralizes privileged AWS mail setup, but mixes application resources into the platform layer and makes app changes depend on platform deploys.
- **Scaffold app code before mail infra permissions** - Enables local code progress, but leaves deployment blocked and invites broad temporary IAM grants.

## Consequences

Project Terraform remains the source of truth for SES, S3, and SNS resources while the deployer role stays least-privilege.
