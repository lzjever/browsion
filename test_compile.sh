#!/bin/bash
echo "=== 检查 Rust 代码语法 ==="
cd src-tauri/src
for file in $(find . -name "*.rs"); do
    echo "检查: $file"
    rustc --crate-type lib $file --out-dir /tmp/rust_check 2>&1 | grep -E "(error|warning)" | head -5
done
echo "=== 检查 TypeScript 代码 ==="
cd ../../src
npx tsc --noEmit 2>&1 | head -20
