// other string functions symbols are offered by Rust compiler-builtins-mem

void *memchr(const void *src, int c, unsigned long n) {
    const unsigned char *s = src;
    c = (unsigned char)c;
    for (; n && *s != c; s++, n--)
        ;
    return n ? (void *)s : 0;
}

static void *memrchr(const void *m, int c, unsigned long n) {
    const unsigned char *s = m;
    c = (unsigned char)c;
    while (n--)
        if (s[n] == c)
            return (void *)(s + n);
    return 0;
}

unsigned long strnlen(const char *s, unsigned long n) {
    unsigned long i;
    for (i = 0; i < n && s[i]; i++)
        ;
    return i;
}

char *strrchr(const char *s, int c) { return memrchr(s, c, strlen(s) + 1); }
