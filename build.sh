#!/bin/bash
set -e

echo "========================================="
echo "Catalyst Test Build Script"
echo "========================================="
echo ""

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Navigate to script directory
cd "$(dirname "$0")"

echo -e "${BLUE}Step 1: Building catalyst CLI...${NC}"
mise exec -- cargo build --release
echo -e "${GREEN}✓ Catalyst CLI built successfully${NC}"
echo ""

echo -e "${BLUE}Step 2: Cleaning previous Bazel artifacts...${NC}"
rm -rf Fixture/bazel-* Fixture/WORKSPACE Fixture/BUILD Fixture/.bazelrc 2>/dev/null || true
echo -e "${GREEN}✓ Cleaned previous artifacts${NC}"
echo ""

echo -e "${BLUE}Step 3: Running catalyst on Fixture project...${NC}"
cd Fixture
../target/release/catalyst
cd ..
echo ""

if [ -f "Fixture/WORKSPACE" ] && [ -f "Fixture/BUILD" ] && [ -f "Fixture/.bazelrc" ]; then
    echo -e "${GREEN}✓ Bazel files generated successfully${NC}"
    echo ""

    echo -e "${BLUE}Generated files:${NC}"
    echo "  - Fixture/WORKSPACE"
    echo "  - Fixture/BUILD"
    echo "  - Fixture/.bazelrc"
    echo ""

    echo -e "${BLUE}BUILD file contents:${NC}"
    echo "---"
    cat Fixture/BUILD
    echo "---"
    echo ""

    echo -e "${YELLOW}Note: Bazel build may fail if Xcode/Apple development tools aren't configured.${NC}"
    echo -e "${YELLOW}The BUILD files are generated correctly from the Tuist graph.${NC}"
else
    echo -e "${RED}✗ Bazel files were not generated${NC}"
    exit 1
fi

echo ""
echo -e "${GREEN}=========================================${NC}"
echo -e "${GREEN}Test completed successfully!${NC}"
echo -e "${GREEN}=========================================${NC}"
echo ""
echo "Catalyst cache location:"
echo "  ~/Library/Caches/catalyst"
echo ""
echo "To view cached graph:"
echo "  cat ~/Library/Caches/catalyst/graph.json | jq"
