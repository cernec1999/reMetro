#!/bin/bash
# Check Python code without making changes (useful for CI)

set -e

echo "🔍 Running Python linting checks (no modifications)..."
echo

# Navigate to the remetro-font directory (parent of bin)
cd "$(dirname "$0")/.."

# Check if virtual environment exists and activate it
if [ -d "venv" ]; then
    echo "📦 Activating virtual environment..."
    source venv/bin/activate
fi

# Install dev dependencies if not already installed
echo "📦 Installing development dependencies..."
pip install -e ".[dev]"

echo
echo "🎨 Checking black formatting..."
black --check --diff src/

echo
echo "📚 Checking isort import order..."
isort --check-only --diff src/

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
echo "✅ All essential linting checks completed!"