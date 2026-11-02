#include <stdlib.h>
#include <vorbis/vorbisfile.h>

/* Allocate with the size and alignment of the linked Vorbis headers. */
void *retrodeck_vorbis_alloc(void) {
    return calloc(1, sizeof(OggVorbis_File));
}
