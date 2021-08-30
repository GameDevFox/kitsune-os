char charAsHex(char input) {
	char binary;
	if(input >= '0' && input <= '9') // Number
		binary = (input - '0');
	else if(input >= 'a' && input <= 'f') // Letter
		binary = (input - 'a' + 10);
	else
		binary = 0xff;

	return binary;
}
