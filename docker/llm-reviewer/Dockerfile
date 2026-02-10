FROM python:3.11-slim

RUN pip install --no-cache-dir httpx>=0.27.0

WORKDIR /app
COPY agent.py /app/agent.py

ENTRYPOINT ["python", "agent.py"]
