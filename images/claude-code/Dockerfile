# CCodeBoX — Claude Code Agent Image
# Built on top of base image
#
# Build from repo root:
#   docker build -t ccodebox-cc:latest -f images/claude-code/Dockerfile .
FROM ccodebox-base:latest

# Install Claude Code CLI
RUN npm install -g @anthropic-ai/claude-code

# Entry script (path relative to repo root build context)
COPY scripts/entrypoint.sh /entrypoint.sh
RUN chmod 755 /entrypoint.sh

USER agent
ENTRYPOINT ["/entrypoint.sh"]
