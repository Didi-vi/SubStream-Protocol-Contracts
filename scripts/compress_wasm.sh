#!/bin/bash

# WASM Compression Script for SubStream Protocol Contracts
# This script uses wasm-opt from Binaryen to optimize WASM binaries for deployment

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default values
CONTRACT_DIR="contracts/substream_contracts"
OUTPUT_DIR="target/compressed"
OPTIMIZATION_LEVEL="Oz"  # Most aggressive optimization

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --contract-dir)
            CONTRACT_DIR="$2"
            shift 2
            ;;
        --output-dir)
            OUTPUT_DIR="$2"
            shift 2
            ;;
        --optimization-level)
            OPTIMIZATION_LEVEL="$2"
            shift 2
            ;;
        --help|-h)
            echo "Usage: $0 [OPTIONS]"
            echo "Options:"
            echo "  --contract-dir DIR     Contract directory (default: contracts/substream_contracts)"
            echo "  --output-dir DIR      Output directory for compressed WASM (default: target/compressed)"
            echo "  --optimization-level L Optimization level (default: Oz)"
            echo "                        Available levels: O0, O1, O2, O3, Os, Oz"
            echo "  --help, -h           Show this help message"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

echo -e "${BLUE}🚀 SubStream Protocol WASM Compression Script${NC}"
echo -e "${BLUE}=============================================${NC}"

# Check if wasm-opt is available
if ! command -v wasm-opt &> /dev/null; then
    echo -e "${RED}❌ Error: wasm-opt not found${NC}"
    echo -e "${YELLOW}Please install Binaryen: brew install binaryen${NC}"
    exit 1
fi

# Check if contract directory exists
if [ ! -d "$CONTRACT_DIR" ]; then
    echo -e "${RED}❌ Error: Contract directory '$CONTRACT_DIR' not found${NC}"
    exit 1
fi

# Change to contract directory
cd "$CONTRACT_DIR"

# Build the contract first
echo -e "${YELLOW}🔨 Building contract...${NC}"
if ! stellar contract build; then
    echo -e "${RED}❌ Build failed${NC}"
    exit 1
fi

# Create output directory
mkdir -p "$OUTPUT_DIR"

# Find WASM files
WASM_DIR="target/wasm32v1-none/release"
if [ ! -d "$WASM_DIR" ]; then
    echo -e "${RED}❌ Error: WASM directory '$WASM_DIR' not found${NC}"
    echo -e "${YELLOW}Make sure the contract was built successfully${NC}"
    exit 1
fi

WASM_FILES=("$WASM_DIR"/*.wasm)
if [ ! -f "${WASM_FILES[0]}" ]; then
    echo -e "${RED}❌ Error: No WASM files found in '$WASM_DIR'${NC}"
    exit 1
fi

echo -e "${YELLOW}📦 Compressing WASM files with optimization level: $OPTIMIZATION_LEVEL${NC}"

total_original=0
total_compressed=0

# Process each WASM file
for wasm_file in "${WASM_FILES[@]}"; do
    if [ -f "$wasm_file" ]; then
        basename=$(basename "$wasm_file" .wasm)
        output_file="$OUTPUT_DIR/${basename}.optimized.wasm"
        
        echo -e "${BLUE}  Optimizing $basename.wasm...${NC}"
        
        # Run wasm-opt with aggressive optimization
        wasm_opt_flags="-$OPTIMIZATION_LEVEL --vacuum --dae --remove-unused-names --remove-unused-types --merge-blocks --simplify-locals --coalesce-locals"
        
        if wasm-opt $wasm_opt_flags "$wasm_file" -o "$output_file"; then
            original_size=$(wc -c < "$wasm_file")
            compressed_size=$(wc -c < "$output_file")
            reduction=$((($original_size - $compressed_size) * 100 / $original_size))
            
            total_original=$((total_original + original_size))
            total_compressed=$((total_compressed + compressed_size))
            
            echo -e "${GREEN}    ✅ Original: $original_size bytes, Compressed: $compressed_size bytes, Reduction: ${reduction}%${NC}"
        else
            echo -e "${RED}    ❌ Failed to optimize $basename.wasm${NC}"
            exit 1
        fi
    fi
done

# Calculate total reduction
total_reduction=$((($total_original - $total_compressed) * 100 / $total_original))

echo -e "${GREEN}🎉 Compression complete!${NC}"
echo -e "${GREEN}📊 Summary:${NC}"
echo -e "${GREEN}   Total original size: $total_original bytes${NC}"
echo -e "${GREEN}   Total compressed size: $total_compressed bytes${NC}"
echo -e "${GREEN}   Total reduction: ${total_reduction}%${NC}"
echo -e "${GREEN}   Compressed files saved to: $OUTPUT_DIR${NC}"

# List compressed files
echo -e "${BLUE}📁 Compressed files:${NC}"
ls -lh "$OUTPUT_DIR"/*.optimized.wasm 2>/dev/null || echo -e "${YELLOW}No compressed files found${NC}"

echo -e "${GREEN}✨ Done! Your optimized WASM files are ready for deployment.${NC}"
