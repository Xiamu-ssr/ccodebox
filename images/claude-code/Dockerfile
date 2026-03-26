# CCodeBoX — Claude Code Agent Image
# Built on top of base image
FROM ccodebox-base:latest

# Install Claude Code CLI
RUN npm install -g @anthropic-ai/claude-code

# Entry script
COPY ../../scripts/entrypoint.sh /entrypoint.sh
RUN chmod 755 /entrypoint.sh

USER agent
ENTRYPOINT ["/entrypoint.sh"]
