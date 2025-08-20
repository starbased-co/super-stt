#!/bin/bash

# Build multi-architecture Docker images for Super STT
# This script uses Docker Buildx to create images for both amd64 and arm64

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

print_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
print_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
print_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Default values
IMAGE_NAME="super-stt"
IMAGE_TAG="latest"
PUSH=false
PLATFORMS="linux/amd64,linux/arm64"

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --push)
            PUSH=true
            shift
            ;;
        --tag)
            IMAGE_TAG="$2"
            shift 2
            ;;
        --name)
            IMAGE_NAME="$2"
            shift 2
            ;;
        --platform)
            PLATFORMS="$2"
            shift 2
            ;;
        --help)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --push              Push the image to registry"
            echo "  --tag TAG           Image tag (default: latest)"
            echo "  --name NAME         Image name (default: super-stt)"
            echo "  --platform PLATFORM Comma-separated platforms (default: linux/amd64,linux/arm64)"
            echo "  --help              Show this help message"
            exit 0
            ;;
        *)
            print_error "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Check if Docker is installed
if ! command -v docker &> /dev/null; then
    print_error "Docker is not installed"
    exit 1
fi

# Check if buildx is available
if ! docker buildx version &> /dev/null; then
    print_error "Docker buildx is not available"
    print_info "Installing buildx..."
    docker buildx install
fi

# Create or use existing buildx builder
BUILDER_NAME="super-stt-builder"
if ! docker buildx ls | grep -q "$BUILDER_NAME"; then
    print_info "Creating buildx builder: $BUILDER_NAME"
    docker buildx create --name "$BUILDER_NAME" --use --platform "$PLATFORMS"
else
    print_info "Using existing buildx builder: $BUILDER_NAME"
    docker buildx use "$BUILDER_NAME"
fi

# Bootstrap the builder
print_info "Bootstrapping builder..."
docker buildx inspect --bootstrap

# Build the image
print_info "Building multi-arch image for platforms: $PLATFORMS"
print_info "Image: ${IMAGE_NAME}:${IMAGE_TAG}"

BUILD_CMD="docker buildx build --platform $PLATFORMS"

if [ "$PUSH" = true ]; then
    BUILD_CMD="$BUILD_CMD --push"
else
    BUILD_CMD="$BUILD_CMD --load"
    # Note: --load only works for single platform, use --output for multi-platform local storage
    if [[ "$PLATFORMS" == *","* ]]; then
        print_warn "Multi-platform build detected. Using local registry output instead of --load"
        BUILD_CMD="docker buildx build --platform $PLATFORMS --output type=oci,dest=${IMAGE_NAME}-${IMAGE_TAG}.tar"
    fi
fi

BUILD_CMD="$BUILD_CMD -t ${IMAGE_NAME}:${IMAGE_TAG} ."

print_info "Running: $BUILD_CMD"
eval $BUILD_CMD

if [ $? -eq 0 ]; then
    print_info "Build completed successfully!"
    
    if [ "$PUSH" = false ] && [[ "$PLATFORMS" == *","* ]]; then
        print_info "Multi-platform image saved to: ${IMAGE_NAME}-${IMAGE_TAG}.tar"
        print_info "To load it: docker load < ${IMAGE_NAME}-${IMAGE_TAG}.tar"
    elif [ "$PUSH" = true ]; then
        print_info "Image pushed to registry: ${IMAGE_NAME}:${IMAGE_TAG}"
    else
        print_info "Image loaded locally: ${IMAGE_NAME}:${IMAGE_TAG}"
    fi
else
    print_error "Build failed!"
    exit 1
fi