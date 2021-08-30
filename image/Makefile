include ../vars.mk

%.data: %.png
	magick $*.png -separate -swap 0,2 -combine rgba:$*.data

%.o: %.data
	$(PREFIX)ld -r -b binary $*.data -o $*.o
