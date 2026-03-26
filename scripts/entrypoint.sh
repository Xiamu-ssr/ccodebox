#!/bin/bash
set -uo pipefail

# ===== Config (injected by orchestrator via env vars) =====
AGENT_TYPE="${AGENT_TYPE:-claude-code}"
TASK_PROMPT="${TASK_PROMPT:-}"
TASK_ID="${TASK_ID:-unknown}"

LOOP_DIR="/workspace/.loop"
mkdir -p "$LOOP_DIR"

START_TIME=$(date +%s)

echo "[loop] ========================================="
echo "[loop] CCodeBoX — Agent Container"
echo "[loop] Agent: $AGENT_TYPE"
echo "[loop] Task ID: $TASK_ID"
echo "[loop] ========================================="

# ===== Phase 1: Setup =====
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

# ===== Phase 2: Prompt Assembly =====
SYSTEM_RULES=""
if [ -f "/system-rules.md" ]; then
    SYSTEM_RULES=$(cat /system-rules.md)
fi

AGENTS_MD=""
if [ -f "AGENTS.md" ]; then
    AGENTS_MD=$(cat AGENTS.md)
fi

FULL_PROMPT=""
[ -n "$SYSTEM_RULES" ] && FULL_PROMPT="$SYSTEM_RULES

"
[ -n "$AGENTS_MD" ] && FULL_PROMPT="${FULL_PROMPT}${AGENTS_MD}

"
FULL_PROMPT="${FULL_PROMPT}${TASK_PROMPT}"

# ===== Phase 3: Run Agent =====
echo "[loop] Running $AGENT_TYPE agent..."

AGENT_EXIT=1
if [ "$AGENT_TYPE" = "claude-code" ]; then
    claude --print \
        --dangerously-skip-permissions \
        --model "${CC_MODEL:-claude-sonnet-4-20250514}" \
        "$FULL_PROMPT" \
        > "$LOOP_DIR/agent.log" 2>&1
    AGENT_EXIT=$?
elif [ "$AGENT_TYPE" = "codex" ]; then
    codex \
        --model "${CODEX_MODEL:-o3-mini}" \
        --quiet \
        --full-auto \
        "$FULL_PROMPT" \
        > "$LOOP_DIR/agent.log" 2>&1
    AGENT_EXIT=$?
else
    echo "[loop] Unknown agent type: $AGENT_TYPE"
    exit 1
fi

echo "[loop] Agent exited with code: $AGENT_EXIT"
echo "[loop] Agent output (last 20 lines):"
tail -20 "$LOOP_DIR/agent.log"

# ===== Phase 4: Collect =====
echo ""
echo "[loop] ===== Collecting Report ====="

END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))

# Stage changes and generate diff
git add -A 2>/dev/null

CHANGED_FILES_JSON="[]"
CHANGED_LIST=$(git diff --cached --name-only 2>/dev/null)
if [ -n "$CHANGED_LIST" ]; then
    CHANGED_FILES_JSON=$(echo "$CHANGED_LIST" | jq -R -s 'split("\n") | map(select(length > 0))')
    git diff --cached > "$LOOP_DIR/diff.patch" 2>/dev/null
fi

LINES_ADDED=$(git diff --cached --numstat 2>/dev/null | awk '{s+=$1}END{print s+0}')
LINES_REMOVED=$(git diff --cached --numstat 2>/dev/null | awk '{s+=$2}END{print s+0}')

HAS_SUMMARY=false
[ -f "$LOOP_DIR/summary.md" ] && HAS_SUMMARY=true

# Determine branch name
CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "")

# Determine model used
AGENT_MODEL=""
if [ "$AGENT_TYPE" = "claude-code" ]; then
    AGENT_MODEL="${CC_MODEL:-claude-sonnet-4-20250514}"
elif [ "$AGENT_TYPE" = "codex" ]; then
    AGENT_MODEL="${CODEX_MODEL:-o3-mini}"
fi

# Write report.json (new format)
cat > "$LOOP_DIR/report.json" << REPORT_EOF
{
    "agent_exit_code": $AGENT_EXIT,
    "has_summary": $HAS_SUMMARY,
    "files_changed": $CHANGED_FILES_JSON,
    "branch": "$CURRENT_BRANCH",
    "duration_seconds": $DURATION,
    "lines_added": $LINES_ADDED,
    "lines_removed": $LINES_REMOVED,
    "model": "$AGENT_MODEL"
}
REPORT_EOF

echo "[loop] Report:"
cat "$LOOP_DIR/report.json"

# Commit changes
if [ -n "$CHANGED_LIST" ]; then
    git commit -m "ccodebox: task $TASK_ID" --allow-empty 2>/dev/null || true
fi

# Push if REPO_URL is set and GITHUB_TOKEN available
PUSHED=false
if [ -n "${REPO_URL:-}" ] && [ -n "${GITHUB_TOKEN:-}" ] && [ -n "$CHANGED_LIST" ]; then
    echo "[loop] Pushing to remote..."
    if git push origin HEAD 2>/dev/null; then
        PUSHED=true
        echo "[loop] Push succeeded"
    else
        echo "[loop] Push failed"
    fi
fi

# Append pushed status to report
python3 -c "
import json
with open('$LOOP_DIR/report.json') as f:
    r = json.load(f)
r['pushed'] = $( [ "$PUSHED" = true ] && echo 'True' || echo 'False' )
with open('$LOOP_DIR/report.json', 'w') as f:
    json.dump(r, f, indent=4)
" 2>/dev/null || true

echo ""
echo "[loop] Files in .loop/:"
ls -la "$LOOP_DIR/"

echo ""
if [ $AGENT_EXIT -eq 0 ]; then
    echo "[loop] Task completed successfully!"
    exit 0
else
    echo "[loop] Task failed (agent exit code: $AGENT_EXIT)"
    exit $AGENT_EXIT
fi
