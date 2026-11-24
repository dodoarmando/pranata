set shell := ["bash", "-euo", "pipefail", "-c"]

# List available recipes.
default:
    @just --list

# Install dependencies (Task 10).
install:
    @echo "install: not implemented yet (Task 10)"

# Run the API and web app in development mode (Task 10).
dev:
    @echo "dev: not implemented yet (Task 10)"

# Start the local database container (Task 3).
db-up:
    @echo "db-up: not implemented yet (Task 3)"

# Apply database migrations (Task 3).
db-migrate:
    @echo "db-migrate: not implemented yet (Task 3)"

# Generate the OpenAPI contract and web client (Task 3).
gen:
    @echo "gen: not implemented yet (Task 3)"

# Run all tests (Task 3).
test:
    @echo "test: not implemented yet (Task 3)"

# Run every check (Task 3).
check:
    @echo "check: not implemented yet (Task 3)"

# Validate commit messages from a ref to HEAD.
check-commits from="origin/main":
    bunx commitlint --from {{ from }} --to HEAD

# Run API checks (Task 3).
check-api:
    @echo "check-api: not implemented yet (Task 3)"

# Run web checks (Task 10).
check-web:
    @echo "check-web: not implemented yet (Task 10)"

# Verify the OpenAPI contract is up to date (Task 10).
check-contract:
    @echo "check-contract: not implemented yet (Task 10)"

# Verify decision-to-test traceability (Task 2).
check-trace:
    @echo "check-trace: not implemented yet (Task 2)"

# Validate infrastructure files (Task 16).
check-infra:
    @echo "check-infra: not implemented yet (Task 16)"

# Run decision traceability in strict mode (Task 2).
check-trace-strict:
    @echo "check-trace-strict: not implemented yet (Task 2)"

# Run API tests (Task 3).
test-api:
    @echo "test-api: not implemented yet (Task 3)"

# Provision one Installation (Task 16).
provision:
    @echo "provision: not implemented yet (Task 16)"

# Fail when real village data leaks outside allowed paths.
check-village-data:
    tools/no-village-data.sh

# Build container images (Task 13).
images:
    @echo "images: not implemented yet (Task 13)"

# Run end-to-end tests (Task 14).
e2e:
    @echo "e2e: not implemented yet (Task 14)"

# Run performance checks (Task 14).
perf:
    @echo "perf: not implemented yet (Task 14)"

# Run load tests (Task 14).
load:
    @echo "load: not implemented yet (Task 14)"

# Load seed data (plan P6).
seed:
    @echo "seed: not implemented yet (plan P6)"

# Deploy the stack (Task 16).
deploy:
    @echo "deploy: not implemented yet (Task 16)"
