FROM node:18-alpine

WORKDIR /app

COPY . .

RUN npm i docsify-cli -g

CMD ["docsify", "serve", "."]