#!/bin/bash

PROJ_DATA=/usr/share/proj ./target/release/mpforge-cli build --config examples/bdtopo-d038-config.yaml --report examples/output/report-d038.json --jobs 4 -vv
