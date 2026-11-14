/* Test stand-in for a compositor's virtual-monitor stream, with a picture.
 *
 * Like Mutter's virtual stream source, it offers a range of sizes and takes
 * whichever one the consumer asks for. Its own preference is 1280x720, so a
 * 1920x1080 result can only come from the consumer's request.
 *
 * The picture has a 12 pixel white edge around a gradient with a bar moving
 * across it, so a screenshot shows whether it is drawn unscaled and whether it
 * is still updating.
 *
 * It prints "node id: <n>" once the stream exists; the mock portal reads that.
 */
#include <stdio.h>
#include <string.h>
#include <time.h>

#include <pipewire/pipewire.h>
#include <spa/param/video/format-utils.h>

struct data {
	struct pw_main_loop *loop;
	struct pw_stream *stream;
	struct spa_source *timer;
	struct spa_video_info_raw format;
	uint32_t stride;
	uint32_t seq;
};

static void on_timeout(void *userdata, uint64_t expirations)
{
	struct data *d = userdata;
	pw_stream_trigger_process(d->stream);
}

static void on_process(void *userdata)
{
	struct data *d = userdata;
	struct pw_buffer *b;
	struct spa_buffer *buf;
	struct spa_meta_header *h;
	uint32_t w = d->format.size.width, ht = d->format.size.height;
	uint32_t x, y;
	uint8_t *p;

	if ((b = pw_stream_dequeue_buffer(d->stream)) == NULL)
		return;
	buf = b->buffer;
	if ((p = buf->datas[0].data) == NULL) {
		pw_stream_queue_buffer(d->stream, b);
		return;
	}

	if ((h = spa_buffer_find_meta_data(buf, SPA_META_Header, sizeof(*h)))) {
		struct timespec ts;
		clock_gettime(CLOCK_MONOTONIC, &ts);
		h->pts = SPA_TIMESPEC_TO_NSEC(&ts);
		h->flags = 0;
		h->seq = d->seq;
		h->dts_offset = 0;
	}

	/* BGRx: a gradient, a white frame along the very edge so cropping would
	 * show, and a bar that moves so a frozen picture would show. */
	uint32_t bar = (d->seq * 16) % w;
	for (y = 0; y < ht; y++) {
		uint8_t *row = p + (size_t)y * d->stride;
		for (x = 0; x < w; x++) {
			uint8_t *px = row + x * 4;
			int edge = x < 12 || y < 12 || x >= w - 12 || y >= ht - 12;
			int in_bar = x >= bar && x < bar + 40;
			if (edge) {
				px[0] = px[1] = px[2] = 255;
			} else if (in_bar) {
				px[0] = 0; px[1] = 220; px[2] = 255;
			} else {
				px[0] = 160;                 /* B */
				px[1] = (uint8_t)(y * 255 / ht); /* G */
				px[2] = (uint8_t)(x * 255 / w);  /* R */
			}
			px[3] = 255;
		}
	}
	d->seq++;

	buf->datas[0].chunk->offset = 0;
	buf->datas[0].chunk->size = ht * d->stride;
	buf->datas[0].chunk->stride = d->stride;
	pw_stream_queue_buffer(d->stream, b);
}

static void on_state_changed(void *userdata, enum pw_stream_state old,
		enum pw_stream_state state, const char *error)
{
	struct data *d = userdata;
	struct timespec timeout = { 0, 0 }, interval = { 0, 0 };

	printf("state: %s%s%s\n", pw_stream_state_as_string(state),
			error ? " error: " : "", error ? error : "");
	switch (state) {
	case PW_STREAM_STATE_PAUSED:
		printf("node id: %u\n", pw_stream_get_node_id(d->stream));
		pw_loop_update_timer(pw_main_loop_get_loop(d->loop), d->timer,
				NULL, NULL, false);
		break;
	case PW_STREAM_STATE_STREAMING:
		timeout.tv_nsec = 1;
		interval.tv_nsec = 33 * SPA_NSEC_PER_MSEC;
		pw_loop_update_timer(pw_main_loop_get_loop(d->loop), d->timer,
				&timeout, &interval, false);
		break;
	case PW_STREAM_STATE_ERROR:
	case PW_STREAM_STATE_UNCONNECTED:
		pw_main_loop_quit(d->loop);
		break;
	default:
		break;
	}
	fflush(stdout);
}

static void on_param_changed(void *userdata, uint32_t id, const struct spa_pod *param)
{
	struct data *d = userdata;
	uint8_t buffer[1024];
	struct spa_pod_builder b = SPA_POD_BUILDER_INIT(buffer, sizeof(buffer));
	const struct spa_pod *params[2];

	if (param == NULL || id != SPA_PARAM_Format)
		return;
	if (spa_format_video_raw_parse(param, &d->format) < 0)
		return;

	d->stride = SPA_ROUND_UP_N(d->format.size.width * 4, 4);
	printf("negotiated size: %ux%u\n", d->format.size.width, d->format.size.height);
	fflush(stdout);

	params[0] = spa_pod_builder_add_object(&b,
		SPA_TYPE_OBJECT_ParamBuffers, SPA_PARAM_Buffers,
		SPA_PARAM_BUFFERS_buffers, SPA_POD_CHOICE_RANGE_Int(8, 2, 16),
		SPA_PARAM_BUFFERS_blocks, SPA_POD_Int(1),
		SPA_PARAM_BUFFERS_size, SPA_POD_Int(d->stride * d->format.size.height),
		SPA_PARAM_BUFFERS_stride, SPA_POD_Int(d->stride));
	params[1] = spa_pod_builder_add_object(&b,
		SPA_TYPE_OBJECT_ParamMeta, SPA_PARAM_Meta,
		SPA_PARAM_META_type, SPA_POD_Id(SPA_META_Header),
		SPA_PARAM_META_size, SPA_POD_Int(sizeof(struct spa_meta_header)));
	pw_stream_update_params(d->stream, params, 2);
}

static const struct pw_stream_events stream_events = {
	PW_VERSION_STREAM_EVENTS,
	.state_changed = on_state_changed,
	.param_changed = on_param_changed,
	.process = on_process,
};

int main(int argc, char *argv[])
{
	struct data d = { 0 };
	uint8_t buffer[1024];
	struct spa_pod_builder b = SPA_POD_BUILDER_INIT(buffer, sizeof(buffer));
	const struct spa_pod *params[1];

	pw_init(&argc, &argv);
	d.loop = pw_main_loop_new(NULL);
	d.stream = pw_stream_new_simple(pw_main_loop_get_loop(d.loop),
			"meeseeks-test-monitor",
			pw_properties_new(PW_KEY_MEDIA_CLASS, "Video/Source", NULL),
			&stream_events, &d);
	d.timer = pw_loop_add_timer(pw_main_loop_get_loop(d.loop), on_timeout, &d);

	params[0] = spa_pod_builder_add_object(&b,
		SPA_TYPE_OBJECT_Format, SPA_PARAM_EnumFormat,
		SPA_FORMAT_mediaType, SPA_POD_Id(SPA_MEDIA_TYPE_video),
		SPA_FORMAT_mediaSubtype, SPA_POD_Id(SPA_MEDIA_SUBTYPE_raw),
		SPA_FORMAT_VIDEO_format, SPA_POD_Id(SPA_VIDEO_FORMAT_BGRx),
		SPA_FORMAT_VIDEO_size, SPA_POD_CHOICE_RANGE_Rectangle(
			&SPA_RECTANGLE(1280, 720),
			&SPA_RECTANGLE(1, 1),
			&SPA_RECTANGLE(8192, 8192)),
		SPA_FORMAT_VIDEO_framerate, SPA_POD_Fraction(&SPA_FRACTION(0, 1)),
		SPA_FORMAT_VIDEO_maxFramerate, SPA_POD_CHOICE_RANGE_Fraction(
			&SPA_FRACTION(60, 1),
			&SPA_FRACTION(1, 1),
			&SPA_FRACTION(60, 1)));

	pw_stream_connect(d.stream, PW_DIRECTION_OUTPUT, PW_ID_ANY,
			PW_STREAM_FLAG_DRIVER | PW_STREAM_FLAG_MAP_BUFFERS,
			params, 1);

	pw_main_loop_run(d.loop);

	pw_stream_destroy(d.stream);
	pw_main_loop_destroy(d.loop);
	pw_deinit();
	return 0;
}
