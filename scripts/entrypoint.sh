#!/bin/bash
set -uo pipefail

# ===== Config (injected by orchestrator via env vars) =====
AGENT_TYPE="${AGENT_TYPE:-claude-code}"
TASK_PROMPT="${TASK_PROMPT:-}"
MAX_ROUNDS="${MAX_ROUNDS:-3}"

LOOP_DIR="/workspace/.loop"
mkdir -p "$LOOP_DIR"

echo "[loop] ========================================="
echo "[loop] Loop POC — Agent Container"
echo "[loop] Agent: $AGENT_TYPE"
echo "[loop] Max rounds: $MAX_ROUNDS"
echo "[loop] ========================================="

# ===== Phase 1: Setup =====
# If REPO_URL is set, clone it. Otherwise work in /workspace directly.
if [ -n "${REPO_URL:-}" ]; then
    echo "[loop] Cloning $REPO_URL ..."
    git clone "$REPO_URL" /workspace/repo
    cd /workspace/repo
    if [ -n "${BRANCH:-}" ]; then
        git checkout -b "$BRANCH" 2>/dev/null || git checkout "$BRANCH"
    fi
else
    echo "[loop] No REPO_URL, working in /workspace"
    cd /workspace
    git init 2>/dev/null || true
fi

# Install deps if present
if [ -f "requirements.txt" ]; then
    echo "[loop] Installing Python deps..."
    pip3 install --break-system-packages -r requirements.txt 2>/dev/null
fi
if [ -f "package.json" ]; then
    echo "[loop] Installing Node deps..."
    npm install 2>/dev/null
fi

# ===== Phase 2: Agent Loop =====
# Build the full prompt with work rules
FULL_PROMPT="$TASK_PROMPT

=== 工作规则（必须遵守）===
1. 完成编码后，运行项目的测试验证你的修改
2. 确保所有测试通过
3. 确保代码风格检查通过（如果有 ruff/eslint）
4. 将你的工作摘要写入 .loop/summary.md，包括做了什么、为什么、遇到的问题
5. 不要修改不相关的文件"

ROUND=1
VERIFY_PASSED=false

while [ $ROUND -le $MAX_ROUNDS ]; do
    echo ""
    echo "[loop] ===== Round $ROUND / $MAX_ROUNDS ====="
    echo "[loop] Running agent..."
    
    if [ "$AGENT_TYPE" = "claude-code" ]; then
        claude --print \
            --dangerously-skip-permissions \
            --model "${CC_MODEL:-claude-sonnet-4-20250514}" \
            "$FULL_PROMPT" \
            > "$LOOP_DIR/agent-round-$ROUND.log" 2>&1
        AGENT_EXIT=$?
    else
        echo "[loop] Unknown agent type: $AGENT_TYPE"
        exit 1
    fi
    
    echo "[loop] Agent exited with code: $AGENT_EXIT"
    echo "[loop] Agent output (last 20 lines):"
    tail -20 "$LOOP_DIR/agent-round-$ROUND.log"
    
    # ===== Phase 3: Wrapper Verification =====
    echo ""
    echo "[loop] ===== Verification ====="
    
    VERIFY_PASSED=true
    LINT_STATUS="skipped"
    TEST_STATUS="skipped"
    
    # L1: Lint (if ruff available and python files exist)
    if command -v ruff &>/dev/null && ls *.py **/*.py 2>/dev/null | head -1 > /dev/null 2>&1; then
        echo "[loop] Running ruff..."
        if ruff check . > "$LOOP_DIR/lint.log" 2>&1; then
            LINT_STATUS="pass"
            echo "[loop] ✅ Lint passed"
        else
            LINT_STATUS="fail"
            VERIFY_PASSED=false
            echo "[loop] ❌ Lint failed"
            tail -10 "$LOOP_DIR/lint.log"
        fi
    else
        echo "[loop] ⬜ Lint skipped (no Python files or ruff)"
    fi
    
    # L2: Unit Test
    if [ -f "pytest.ini" ] || [ -f "pyproject.toml" ] || [ -d "tests" ]; then
        echo "[loop] Running pytest..."
        if pytest tests/ -v --tb=short > "$LOOP_DIR/test.log" 2>&1; then
            TEST_STATUS="pass"
            echo "[loop] ✅ Tests passed"
        else
            TEST_STATUS="fail"
            VERIFY_PASSED=false
            echo "[loop] ❌ Tests failed"
            tail -20 "$LOOP_DIR/test.log"
        fi
    elif [ -f "package.json" ] && grep -q '"test"' package.json 2>/dev/null; then
        echo "[loop] Running npm test..."
        if npm test > "$LOOP_DIR/test.log" 2>&1; then
            TEST_STATUS="pass"
            echo "[loop] ✅ Tests passed"
        else
            TEST_STATUS="fail"
            VERIFY_PASSED=false
            echo "[loop] ❌ Tests failed"
            tail -20 "$LOOP_DIR/test.log"
        fi
    else
        echo "[loop] ⬜ Tests skipped (no test config found)"
    fi
    
    # Check results
    if [ "$VERIFY_PASSED" = true ]; then
        echo ""
        echo "[loop] ✅✅✅ All verifications passed on round $ROUND ✅✅✅"
        break
    fi
    
    if [ $ROUND -lt $MAX_ROUNDS ]; then
        echo ""
        echo "[loop] 🔄 Feeding errors back to agent for round $((ROUND + 1))..."
        
        ERROR_CONTEXT=""
        [ -f "$LOOP_DIR/lint.log" ] && [ "$LINT_STATUS" = "fail" ] && \
            ERROR_CONTEXT="$ERROR_CONTEXT\n=== Lint Errors ===\n$(tail -30 $LOOP_DIR/lint.log)"
        [ -f "$LOOP_DIR/test.log" ] && [ "$TEST_STATUS" = "fail" ] && \
            ERROR_CONTEXT="$ERROR_CONTEXT\n=== Test Errors ===\n$(tail -50 $LOOP_DIR/test.log)"
        
        FULL_PROMPT="上一轮的修改未通过验证，请修复以下错误：
$ERROR_CONTEXT

原始任务：
$TASK_PROMPT

=== 工作规则（必须遵守）===
1. 只修复上面列出的错误，不要重写整个项目
2. 修复后运行测试确认通过
3. 更新 .loop/summary.md"
    fi
    
    ROUND=$((ROUND + 1))
done

# ===== Phase 4: Collect Report =====
echo ""
echo "[loop] ===== Collecting Report ====="

# Generate diff
git add -A 2>/dev/null
git diff --cached > "$LOOP_DIR/diff.patch" 2>/dev/null
CHANGED_FILES=$(git diff --cached --name-only 2>/dev/null | tr '\n' ', ')
LINES_ADDED=$(git diff --cached --numstat 2>/dev/null | awk '{s+=$1}END{print s+0}')
LINES_REMOVED=$(git diff --cached --numstat 2>/dev/null | awk '{s+=$2}END{print s+0}')

# Write report
cat > "$LOOP_DIR/report.json" << REPORT_EOF
{
    "verify_passed": $VERIFY_PASSED,
    "rounds": $ROUND,
    "max_rounds": $MAX_ROUNDS,
    "agent_type": "$AGENT_TYPE",
    "lint_status": "$LINT_STATUS",
    "test_status": "$TEST_STATUS",
    "files_changed": "$CHANGED_FILES",
    "lines_added": $LINES_ADDED,
    "lines_removed": $LINES_REMOVED
}
REPORT_EOF

echo "[loop] Report:"
cat "$LOOP_DIR/report.json"

echo ""
echo "[loop] Files in .loop/:"
ls -la "$LOOP_DIR/"

echo ""
if [ "$VERIFY_PASSED" = true ]; then
    echo "[loop] 🎉 Task completed successfully!"
    exit 0
else
    echo "[loop] 💥 Task failed after $MAX_ROUNDS rounds"
    exit 1
fi
