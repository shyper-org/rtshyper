void *memcpy(void *dst, const void *src, unsigned long count);

int memcmp(const void *vl, const void *vr, unsigned long n) {
    const unsigned char *l = vl, *r = vr;
    for (; n && *l == *r; n--, l++, r++)
        ;
    return n ? *l - *r : 0;
}

void *memchr(const void *src, int c, unsigned long n) {
    const unsigned char *s = src;
    c = (unsigned char)c;
    for (; n && *s != c; s++, n--)
        ;
    return n ? (void *)s : 0;
}

void *memmove(void *dest, const void *src, unsigned long n) {
    char *d = dest;
    const char *s = src;

    if (d == s)
        return d;
    if ((unsigned long)s - (unsigned long)d - n <= -2 * n)
        return memcpy(d, s, n);

    if (d < s) {
        for (; n; n--)
            *d++ = *s++;
    } else {
        while (n)
            n--, d[n] = s[n];
    }

    return dest;
}

unsigned long strlen(const char *s) {
    const char *a = s;
    for (; *s; s++)
        ;
    return s - a;
}

unsigned long strnlen(const char *s, unsigned long n) {
    const char *p = memchr(s, 0, n);
    return p ? (unsigned long)(p - s) : n;
}

static char *strchrnul(const char *s, int c) {
    c = (unsigned char)c;
    if (!c)
        return (char *)s + strlen(s);
    for (; *s && *(unsigned char *)s != c; s++)
        ;
    return (char *)s;
}

char *strchr(const char *s, int c) {
    char *r = strchrnul(s, c);
    return *(unsigned char *)r == (unsigned char)c ? r : 0;
}

static void *memrchr(const void *m, int c, unsigned long n) {
    const unsigned char *s = m;
    c = (unsigned char)c;
    while (n--)
        if (s[n] == c)
            return (void *)(s + n);
    return 0;
}


char *strrchr(const char *s, int c) { return memrchr(s, c, strlen(s) + 1); }


