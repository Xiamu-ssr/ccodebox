# CCodeBoX — Codex Agent Image
# Built on top of base image
#
# Build from repo root:
#   docker build -t ccodebox-codex:latest -f images/codex/Dockerfile .
FROM ccodebox-base:latest

# Install Codex CLI
RUN npm install -g @openai/codex

# Entry script + system rules (paths relative to repo root build context)
COPY scripts/entrypoint.sh /entrypoint.sh
COPY scripts/system-rules.md /system-rules.md
RUN chmod 755 /entrypoint.sh

USER agent
ENTRYPOINT ["/entrypoint.sh"]
