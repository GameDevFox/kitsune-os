#define VT_SAVE "\e7"
#define VT_LOAD "\e8"

#define VT_GET_POS "\e[6n"

#define VT_HOME "\e[H"

#define VT_UP(count) "\e[" #count "A"
#define VT_DOWN(count) "\e[" #count "B"

#define VT_RESET  "\e[0m"
#define VT_BRIGHT "\e[1m"
#define VT_DIM    "\e[2m"

#define VT_BLACK   "\e[30m"
#define VT_RED     "\e[31m"
#define VT_GREEN   "\e[32m"
#define VT_YELLOW  "\e[33m"
#define VT_BLUE    "\e[34m"
#define VT_MAGENTA "\e[35m"
#define VT_CYAN    "\e[36m"
#define VT_WHITE   "\e[37m"
#define VT_DEFAULT "\e[39m"

#define VT_BG_BLACK   "\e[40m"
#define VT_BG_RED     "\e[41m"
#define VT_BG_GREEN   "\e[42m"
#define VT_BG_YELLOW  "\e[43m"
#define VT_BG_BLUE    "\e[44m"
#define VT_BG_MAGENTA "\e[45m"
#define VT_BG_CYAN    "\e[46m"
#define VT_BG_WHITE   "\e[47m"
#define VT_BG_DEFAULT "\e[49m"
