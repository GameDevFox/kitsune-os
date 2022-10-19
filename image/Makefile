include ../vars.mk

%.o: %.png
	./image-size-gen $*.png

	magick $*.png -separate -swap 0,2 -combine rgba:- >> $*.png.data
	mv $*.png.data $*.data

	$(PREFIX)ld -r -b binary $*.data -o $*.o
