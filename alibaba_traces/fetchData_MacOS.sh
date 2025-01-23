#!/bin/bash

url='http://aliopentrace.oss-cn-beijing.aliyuncs.com/v2021MicroservicesTraces'

mkdir -p data
cd data || exit

mkdir -p Node
mkdir -p MSResource
mkdir -p MSRTQps
mkdir -p MSCallGraph

cd Node || exit

# Download Node_0.tar.gz
curl -C - --retry 0 --retry-delay 5 --retry-max-time 50 -O "${url}/node/Node_0.tar.gz"

cd ../MSRTQps || exit
# Download MSRTQps files
for ((i=0; i<=1; i++)); do
    curl -C - --retry 0 --retry-delay 5 --retry-max-time 50 -O "${url}/MSRTQps/MSRTQps_${i}.tar.gz"
done

cd ../MSCallGraph || exit
# Download MSCallGraph files
for ((i=0; i<=1; i++)); do
    curl -C - --retry 0 --retry-delay 5 --retry-max-time 50 -O "${url}/MSCallGraph/MSCallGraph_${i}.tar.gz"
done

cd ../MSResource || exit
# Download MSResource files
for ((i=0; i<=1; i++)); do
    curl -C - --retry 0 --retry-delay 5 --retry-max-time 50 -O "${url}/MSResource/MSResource_${i}.tar.gz"
done
