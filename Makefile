.PHONY: setup up down logs build register test clean sdk-build sdk-test sdk-python health status shell-conduit help

COMPOSE := docker compose

help: ## Show all available commands
	@echo "Constellation A2A - Available Commands"
	@echo "======================================="
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  make %-16s %s\n", $$1, $$2}'

setup: ## Run initial setup (build Conduit, generate secrets)
	./scripts/setup.sh

up: ## Start all services in the background
	$(COMPOSE) up -d

down: ## Stop all services
	$(COMPOSE) down

logs: ## Follow logs from all services
	$(COMPOSE) logs -f

build: ## Build all Docker images
	$(COMPOSE) build

register: ## Register agent accounts on Conduit
	./scripts/register-agents.sh

test: ## Run integration tests
	./tests/integration/run_tests.sh

clean: ## Stop all services and remove volumes
	$(COMPOSE) down -v

sdk-build: ## Build the Rust SDK
	cd sdk && cargo build

sdk-test: ## Run Rust SDK tests
	cd sdk && cargo test

sdk-python: ## Build Python bindings in development mode
	cd sdk/constellation-py && maturin develop

health: ## Check Conduit server health
	./scripts/health-check.sh

status: ## Show running containers and their status
	$(COMPOSE) ps

shell-conduit: ## Open a shell in the Conduit container
	$(COMPOSE) exec conduit /bin/sh
