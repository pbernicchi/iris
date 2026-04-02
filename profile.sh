#!/bin/bash
PERFFLAGS="-F 99 -g --call-graph dwarf" cargo flamegraph --profile profiling --bin iris