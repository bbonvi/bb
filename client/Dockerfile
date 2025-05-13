FROM node AS builder 


WORKDIR /app

COPY package.json yarn.lock ./
RUN yarn
COPY . .
RUN yarn build

FROM alpine as runtime

RUN apt update -qq \
  && apt install -qq nginx -y \
  && apt-get clean \
  && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/build/* /var/www/html/

COPY nginx.conf /etc/nginx/sites-enabled/default
EXPOSE 80
CMD ["nginx", "-g", "daemon off;"]
