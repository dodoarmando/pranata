set shell := ["bash", "-euo", "pipefail", "-c"]

# List available recipes.
default:
    @just --list

# Install dependencies (Task 6).
install:
    @echo "install: not implemented yet (Task 6)"

# Run the API and web app in development mode (Task 6).
dev:
    @echo "dev: not implemented yet (Task 6)"

# Start the local database container (Task 4).
db-up:
    @echo "db-up: not implemented yet (Task 4)"

# Apply database migrations (Task 4).
db-migrate:
    @echo "db-migrate: not implemented yet (Task 4)"

# Generate the OpenAPI contract and web client (Task 5).
gen:
    @echo "gen: not implemented yet (Task 5)"

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

# Run web checks (Task 6).
check-web:
    @echo "check-web: not implemented yet (Task 6)"

# Verify the OpenAPI contract is up to date (Task 5).
check-contract:
    @echo "check-contract: not implemented yet (Task 5)"

# Verify decision-to-test traceability (Task 8).
check-trace:
    @echo "check-trace: not implemented yet (Task 8)"

# Validate infrastructure files (Task 7).
check-infra:
    @echo "check-infra: not implemented yet (Task 7)"

# Fail when real village data leaks outside allowed paths.
check-village-data:
    tools/no-village-data.sh

# Build container images (Task 7).
images:
    @echo "images: not implemented yet (Task 7)"

# Run end-to-end tests (Task 9).
e2e:
    @echo "e2e: not implemented yet (Task 9)"

# Run performance checks (Task 9).
perf:
    @echo "perf: not implemented yet (Task 9)"

# Run load tests (Task 9).
load:
    @echo "load: not implemented yet (Task 9)"

# Load seed data (Task 4).
seed:
    @echo "seed: not implemented yet (Task 4)"

# Deploy the stack (Task 7).
deploy:
    @echo "deploy: not implemented yet (Task 7)"
