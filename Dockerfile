FROM alpine:3

RUN apk update && apk add \
    bash \
    gcc-arm-none-eabi \
    imagemagick \
    make

CMD ["/bin/bash", "-c", "make --directory=/kitsune-os"]
