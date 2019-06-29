Compiling ROMs
==============

Example:

```
rgbasm -o lag.o input-lag.s && rgblink -o lag.gb lag.o && rgbfix -v -p 0 lag.gb
```
