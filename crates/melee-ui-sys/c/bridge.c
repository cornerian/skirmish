#include <stdint.h>

int mn_8022C010(int menu_kind, int selection);
int mn_GetDigitAt(int number, int digit);

/* Runtime dependency of the original mn_GetDigitAt definition. */
int powi(int base, int exponent)
{
    int result = 1;
    while (exponent-- > 0) {
        result *= base;
    }
    return result;
}

int32_t skirmish_mn_light_color_index(uint8_t menu_kind, uint16_t selection)
{
    return mn_8022C010(menu_kind, selection);
}

int32_t skirmish_mn_digit_at(int32_t number, int32_t digit)
{
    return mn_GetDigitAt(number, digit);
}
