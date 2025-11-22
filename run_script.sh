#!/bin/bash

# 

cd python_tool_executor

./build_script.sh

cd ../python_tool_server

cargo build --release

cd ..

cargo build --release

# ./python_tool_server 4096 4100 4

bash -c "exec -a proxima_python ./python_tool_server/target/release/python_tool_server" &

# ./proxima_backend_server test test /home/pir/ia/proxima_testing_grounds http://localhost:5001/v1/ 8082

bash -c "exec -a proxima_backend ./target/release/proxima_backend" &