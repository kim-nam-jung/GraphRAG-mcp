#!/bin/bash
fuser -k 3000/tcp
ss -tuln | grep 3000
