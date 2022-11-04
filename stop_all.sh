#!/bin/bash

echo "M140 S0" > /dev/ttyACM0
echo "M104 S0" > /dev/ttyACM0
echo "M106 S0" > /dev/ttyACM0
echo "M84" > /dev/ttyACM0