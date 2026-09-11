#!/bin/bash

# Exit on any error
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m' # No Color

# Track if any typecheck fails
FAILED=0

# Check if a specific test case was provided
# Skip the "--" argument if present (from pnpm run)
TEST_CASE="$1"
if [ "$1" = "--" ]; then
    TEST_CASE="$2"
fi

if [ -n "$TEST_CASE" ]; then
    # Specific test case requested
    TEST_DIR="test/$TEST_CASE"
    
    if [ ! -d "$TEST_DIR" ]; then
        echo -e "${RED}Error: Test directory '$TEST_DIR' does not exist${NC}"
        echo ""
        echo "Available test cases:"
        for dir in test/*/; do
            if [ -f "$dir/tsconfig.json" ]; then
                basename "$dir"
            fi
        done
        exit 1
    fi
    
    if [ ! -f "$TEST_DIR/tsconfig.json" ]; then
        echo -e "${YELLOW}Warning: No tsconfig.json found in '$TEST_DIR'${NC}"
        exit 1
    fi
    
    echo "Running TypeScript type checking on $TEST_DIR..."
    echo ""
    
    if pnpm tsc --noEmit -p "$TEST_DIR"; then
        echo -e "${GREEN}✓${NC} $TEST_DIR passed"
        exit 0
    else
        echo -e "${RED}✗${NC} $TEST_DIR failed"
        exit 1
    fi
else
    # Run on all test directories
    echo "Running TypeScript type checking on all test directories..."
    echo ""
    
    for dir in test/*/; do
        if [ -f "$dir/tsconfig.json" ]; then
            echo "Checking $dir..."
            if pnpm tsc --noEmit -p "$dir"; then
                echo -e "${GREEN}✓${NC} $dir passed"
            else
                echo -e "${RED}✗${NC} $dir failed"
                FAILED=1
            fi
            echo ""
        fi
    done
    
    if [ $FAILED -eq 0 ]; then
        echo -e "${GREEN}All type checks passed!${NC}"
        exit 0
    else
        echo -e "${RED}Some type checks failed${NC}"
        exit 1
    fi
fi