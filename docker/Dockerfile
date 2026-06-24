FROM node:22-bookworm AS web-build

WORKDIR /app/web
COPY web/package.json web/package-lock.json ./
RUN npm ci
COPY web/ ./
RUN npm run build

FROM python:3.12-slim AS runtime

ENV PYTHONUNBUFFERED=1
WORKDIR /app

COPY python ./python
RUN pip install --no-cache-dir -e ./python uvicorn

COPY --from=web-build /app/web/dist ./web-dist

ENV GA3D_STATIC_DIR=/app/web-dist
EXPOSE 8000

CMD ["uvicorn", "ga3d.api:app", "--host", "0.0.0.0", "--port", "8000", "--app-dir", "/app/python"]
