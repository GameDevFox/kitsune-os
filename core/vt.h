#include <stddef.h>

#ifdef TERM_CONTROL_CODES
  #define VT_SAVE "\e7"
  #define VT_LOAD "\e8"

  #define VT_HOME "\e[H"
  #define VT_SET_POS(column, row) "\e[" #row ";" #column "H"
  #define VT_GET_POS "\e[6n"

  #define VT_UP(count) "\e[" #count "A"
  #define VT_DOWN(count) "\e[" #count "B"
  #define VT_RIGHT(count) "\e[" #count "C"
  #define VT_LEFT(count) "\e[" #count "D"

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
#else
  #define VT_SAVE ""
  #define VT_LOAD ""

  #define VT_HOME ""
  #define VT_SET_POS(column, row) ""
  #define VT_GET_POS ""

  #define VT_UP(count) ""
  #define VT_DOWN(count) ""
  #define VT_RIGHT(count) ""
  #define VT_LEFT(count) ""

  #define VT_RESET  ""
  #define VT_BRIGHT ""
  #define VT_DIM    ""

  #define VT_BLACK   ""
  #define VT_RED     ""
  #define VT_GREEN   ""
  #define VT_YELLOW  ""
  #define VT_BLUE    ""
  #define VT_MAGENTA ""
  #define VT_CYAN    ""
  #define VT_WHITE   ""
  #define VT_DEFAULT ""

  #define VT_BG_BLACK   ""
  #define VT_BG_RED     ""
  #define VT_BG_GREEN   ""
  #define VT_BG_YELLOW  ""
  #define VT_BG_BLUE    ""
  #define VT_BG_MAGENTA ""
  #define VT_BG_CYAN    ""
  #define VT_BG_WHITE   ""
  #define VT_BG_DEFAULT ""
#endif

struct VT_Size {
  size_t row;
  size_t column;
};

struct VT_Size vt_get_pos();
void vt_set_pos(int column, int row);
struct VT_Size vt_get_size();
void vt_fill(char c);
