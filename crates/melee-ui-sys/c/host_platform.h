#ifndef SKIRMISH_MELEE_HOST_PLATFORM_H
#define SKIRMISH_MELEE_HOST_PLATFORM_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

/* Replace only platform-width and compiler definitions. Melee declarations
 * and function bodies continue to come from the pinned upstream headers/source. */
#define _DOLPHIN_TYPES_H_
#define RUNTIME_PLATFORM_H
#define _DOLPHIN_OSRTC_H_

typedef int8_t s8;
typedef uint8_t u8;
typedef int16_t s16;
typedef uint16_t u16;
typedef int32_t s32;
typedef uint32_t u32;
typedef int64_t s64;
typedef uint64_t u64;
typedef float f32;
typedef double f64;
typedef volatile f32 vf32;
typedef volatile f64 vf64;
typedef char* Ptr;
typedef int BOOL;
typedef int enum_t;
typedef int32_t melee_ssize_t;
#define ssize_t melee_ssize_t
typedef void (*Event)(void);
typedef bool (*Predicate)(void);

#define FALSE 0
#define TRUE 1
#ifndef NULL
#define NULL ((void*) 0)
#endif
#define ARRAY_SIZE(array) (sizeof(array) / sizeof((array)[0]))
#define ATTRIBUTE_ALIGN(number) __attribute__((aligned(number)))
#define UNUSED __attribute__((unused))
#define ATTRIBUTE_NORETURN __attribute__((noreturn))
#define ATTRIBUTE_RESTRICT __restrict
#define ASM
#define SECTION_INIT
#define SECTION_CTORS
#define SECTION_DTORS
#define SDATA
#define STATIC_ASSERT(condition)
#define ASSERT_SIZE(expression, size)

/* glibc exports an incompatible internal symbol with this name. Rename the
 * Melee declaration and every pinned-source call at preprocessing time. */
#define __assert skirmish_mn_assert

#endif
