#!/bin/bash

image_name=beanbubger/johnson-rs:latest

docker build . -t $image_name 
docker image push $image_name

image_digest="$(docker inspect --format='{{index .RepoDigests 0}}' $image_name)"

ssh root@$HOST_IP docker system prune -a -f
ssh root@$HOST_IP dokku git:from-image johnson-rs $image_digest
