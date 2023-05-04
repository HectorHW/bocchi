#include "stdio.h"
#include "string.h"

int read_input(char* buf) { return gets(buf); }

int inner() {
    char buffer[64];
    read_input(buffer);

    if (strlen(buffer) == 0) {
        puts("got empty string\n");
        return 1;
    }

    if (buffer[4] > buffer[5]) {
        puts("condition is true\n");
        return 10;
    }

    if (strlen(buffer) >= 62) {
        puts("got large string, stack is probably dead, nice!\n");
        return 20;
    }

    puts("got normal string");
    return 0;
}

int main() { return inner(); }