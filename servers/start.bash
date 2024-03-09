#!/bin/bash
cd ./1
python3 -m http.server 8001&
cd ../2
python3 -m http.server 8002&
cd ../3
python3 -m http.server 8003&
cd ../4
python3 -m http.server 8004&
cd ../5
python3 -m http.server 8005&
