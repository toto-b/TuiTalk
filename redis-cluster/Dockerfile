FROM redis:7.2-alpine

RUN mkdir -p /usr/local/etc/redis

COPY redis-cluster.tmpl.conf /usr/local/etc/redis/redis.conf

CMD ["redis-server", "/usr/local/etc/redis/redis.conf"]
