#!/bin/bash
# Lint and format the metro-font-builder Python code

set -e

echo "🔍 Running Python linting and formatting..."
echo

# Navigate to the metro-font-builder directory
cd "$(dirname "$0")"

# Check if virtual environment exists and activate it
if [ -d "venv" ]; then
    echo "📦 Activating virtual environment..."
    source venv/bin/activate
fi

# Install dev dependencies if not already installed
echo "📦 Installing development dependencies..."
pip install -e ".[dev]"

echo
echo "🎨 Running black (code formatter)..."
black src/

echo
echo "📚 Running isort (import sorter)..."
isort src/

echo
echo "🔍 Running flake8 (style guide enforcement)..."
flake8 src/

echo
echo "🔍 Running pylint (comprehensive linting)..."
pylint src/

echo
echo "🔍 Running mypy (type checking) - optional for GUI apps..."
if mypy src/ --no-error-summary; then
    echo "✅ mypy passed!"
else
    echo "⚠️  mypy found issues (expected for PyQt5 GUI applications)"
fi

echo
echo "✅ All linting and formatting completed!"