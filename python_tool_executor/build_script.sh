#!/bin/bash

# $PROXIMA_REPO_LOCATION refers to the absolute path to the root of the proxima_backend git repository

docker build -t proxima_python_executor:any -f $PROXIMA_REPO_LOCATION/python_tool_executor/Dockerfile $PROXIMA_REPO_LOCATION/python_tool_executor/ || exit $?