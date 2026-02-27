#!/bin/bash

# $PROXIMA_REPO_LOCATION refers to the absolute path to the root of the proxima_backend git repository

docker build -t proxima_backend_server:any -f $PROXIMA_REPO_LOCATION/proxima_backend_server/Dockerfile $PROXIMA_REPO_LOCATION/ || exit $?