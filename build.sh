#!/bin/bash

docker build . -t beanbubger/johnson-rs:latest
docker image push beanbubger/johnson-rs:latest
