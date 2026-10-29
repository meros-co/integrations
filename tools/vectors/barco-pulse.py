# Barco Pulse API (barco-pulse): one vector per command. Each request is the
# JSON-RPC 2.0 object of the Pulse catalogs' Service API (property.get,
# property.set, property.subscribe, signal.subscribe, a method call by its
# dotted name), written here with json.dumps, compact, with "id":1 and no
# framing (the API delimits nothing). Replies are the catalogs' examples:
# {"jsonrpc":"2.0","result":...,"id":N}, and JSON-RPC errors.
# The set_<property> commands are listed with their property and an example
# value of the catalog's type and range.
import json as _json
BP = "barco-pulse"


# (command, input, property, the value as its JSON text on the wire)
_SETTERS = [
    ('set_dmx_artnet', {'value': True}, 'dmx.artnet', 'true'),
    ('set_dmx_artnetnet', {'value': 1}, 'dmx.artnetnet', '1'),
    ('set_dmx_artnetuniverse', {'value': 1}, 'dmx.artnetuniverse', '1'),
    ('set_dmx_mode', {'value': 'Example 1'}, 'dmx.mode', '"Example 1"'),
    ('set_dmx_shutdown', {'value': True}, 'dmx.shutdown', 'true'),
    ('set_dmx_shutdowntimeout', {'value': 2}, 'dmx.shutdowntimeout', '2'),
    ('set_dmx_startchannel', {'value': 2}, 'dmx.startchannel', '2'),
    ('set_firmware_eula_termagreement', {'value': True}, 'firmware.eula.termagreement', 'true'),
    ('set_gpio_gpo_mode', {'output': 'trigger1', 'value': 'PulseTrain'}, 'gpio.gpo.trigger1.mode', '"PulseTrain"'),
    ('set_gpio_gpo_value', {'output': 'trigger1', 'value': True}, 'gpio.gpo.trigger1.value', 'true'),
    ('set_illumination_clo_enable', {'value': True}, 'illumination.clo.enable', 'true'),
    ('set_illumination_clo_scale', {'value': 0.5}, 'illumination.clo.scale', '0.50'),
    ('set_illumination_clo_setpoint', {'value': 0.5}, 'illumination.clo.setpoint', '0.50'),
    ('set_illumination_laser_colorratioregulation', {'value': True}, 'illumination.laser.colorratioregulation', 'true'),
    ('set_illumination_nosignal_dimming_enable', {'value': True}, 'illumination.nosignal.dimming.enable', 'true'),
    ('set_illumination_sources_power', {'light_source': 'irled', 'value': 0.5}, 'illumination.sources.irled.power', '0.50'),
    ('set_image_blackcontentdetection_dimminginterval', {'value': 1}, 'image.blackcontentdetection.dimminginterval', '1'),
    ('set_image_blackcontentdetection_enable', {'value': True}, 'image.blackcontentdetection.enable', 'true'),
    ('set_image_blackcontentdetection_sampleinterval', {'value': 501}, 'image.blackcontentdetection.sampleinterval', '501'),
    ('set_image_blackcontentdetection_threshold', {'value': 1}, 'image.blackcontentdetection.threshold', '1'),
    ('set_image_brightness', {'value': 0.0}, 'image.brightness', '0.00'),
    ('set_image_brilliantcolor_mode', {'value': 'Example 1'}, 'image.brilliantcolor.mode', '"Example 1"'),
    ('set_image_capture_enable', {'value': True}, 'image.capture.enable', 'true'),
    ('set_image_color_p7_custom_bluegain', {'value': 0.5}, 'image.color.p7.custom.bluegain', '0.50'),
    ('set_image_color_p7_custom_bluelum', {'value': 0.5}, 'image.color.p7.custom.bluelum', '0.5000'),
    ('set_image_color_p7_custom_bluex', {'value': 0.5}, 'image.color.p7.custom.bluex', '0.5000'),
    ('set_image_color_p7_custom_bluey', {'value': 0.5}, 'image.color.p7.custom.bluey', '0.5000'),
    ('set_image_color_p7_custom_cyangain', {'value': 0.5}, 'image.color.p7.custom.cyangain', '0.50'),
    ('set_image_color_p7_custom_cyanlum', {'value': 0.5}, 'image.color.p7.custom.cyanlum', '0.5000'),
    ('set_image_color_p7_custom_cyanx', {'value': 0.5}, 'image.color.p7.custom.cyanx', '0.5000'),
    ('set_image_color_p7_custom_cyany', {'value': 0.5}, 'image.color.p7.custom.cyany', '0.5000'),
    ('set_image_color_p7_custom_greengain', {'value': 0.5}, 'image.color.p7.custom.greengain', '0.50'),
    ('set_image_color_p7_custom_greenlum', {'value': 0.5}, 'image.color.p7.custom.greenlum', '0.5000'),
    ('set_image_color_p7_custom_greenx', {'value': 0.5}, 'image.color.p7.custom.greenx', '0.5000'),
    ('set_image_color_p7_custom_greeny', {'value': 0.5}, 'image.color.p7.custom.greeny', '0.5000'),
    ('set_image_color_p7_custom_magentagain', {'value': 0.5}, 'image.color.p7.custom.magentagain', '0.50'),
    ('set_image_color_p7_custom_magentalum', {'value': 0.5}, 'image.color.p7.custom.magentalum', '0.5000'),
    ('set_image_color_p7_custom_magentax', {'value': 0.5}, 'image.color.p7.custom.magentax', '0.5000'),
    ('set_image_color_p7_custom_magentay', {'value': 0.5}, 'image.color.p7.custom.magentay', '0.5000'),
    ('set_image_color_p7_custom_mode', {'value': 'Example 1'}, 'image.color.p7.custom.mode', '"Example 1"'),
    ('set_image_color_p7_custom_redgain', {'value': 0.5}, 'image.color.p7.custom.redgain', '0.50'),
    ('set_image_color_p7_custom_redlum', {'value': 0.5}, 'image.color.p7.custom.redlum', '0.5000'),
    ('set_image_color_p7_custom_redx', {'value': 0.5}, 'image.color.p7.custom.redx', '0.5000'),
    ('set_image_color_p7_custom_redy', {'value': 0.5}, 'image.color.p7.custom.redy', '0.5000'),
    ('set_image_color_p7_custom_whitegain', {'value': 0.5}, 'image.color.p7.custom.whitegain', '0.50'),
    ('set_image_color_p7_custom_whitelum', {'value': 0.5}, 'image.color.p7.custom.whitelum', '0.5000'),
    ('set_image_color_p7_custom_whitemode', {'value': 'Temperature'}, 'image.color.p7.custom.whitemode', '"Temperature"'),
    ('set_image_color_p7_custom_whitetemperature', {'value': 3201}, 'image.color.p7.custom.whitetemperature', '3201'),
    ('set_image_color_p7_custom_whitex', {'value': 0.5}, 'image.color.p7.custom.whitex', '0.5000'),
    ('set_image_color_p7_custom_whitey', {'value': 0.5}, 'image.color.p7.custom.whitey', '0.5000'),
    ('set_image_color_p7_custom_yellowgain', {'value': 0.5}, 'image.color.p7.custom.yellowgain', '0.50'),
    ('set_image_color_p7_custom_yellowlum', {'value': 0.5}, 'image.color.p7.custom.yellowlum', '0.5000'),
    ('set_image_color_p7_custom_yellowx', {'value': 0.5}, 'image.color.p7.custom.yellowx', '0.5000'),
    ('set_image_color_p7_custom_yellowy', {'value': 0.5}, 'image.color.p7.custom.yellowy', '0.5000'),
    ('set_image_color_rgbmode_rgbmode', {'value': 'BlueRed'}, 'image.color.rgbmode.rgbmode', '"BlueRed"'),
    ('set_image_connector_colorprimaries', {'connector': 'displayport', 'value': 'DCI-P3-Theater'}, 'image.connector.displayport.colorprimaries', '"DCI-P3-Theater"'),
    ('set_image_connector_colorspace', {'connector': 'displayport', 'value': 'REC2020'}, 'image.connector.displayport.colorspace', '"REC2020"'),
    ('set_image_connector_edid_selected', {'connector': 'displayport', 'value': 'Example 1'}, 'image.connector.displayport.edid.selected', '"Example 1"'),
    ('set_image_connector_signalrange', {'connector': 'displayport', 'value': '16-235'}, 'image.connector.displayport.signalrange', '"16-235"'),
    ('set_image_connector_colorcomponent_blue', {'connector': 'displayport1', 'value': 'BLUE_IN'}, 'image.connector.displayport1.colorcomponent.blue', '"BLUE_IN"'),
    ('set_image_connector_colorcomponent_green', {'connector': 'displayport1', 'value': 'BLUE_IN'}, 'image.connector.displayport1.colorcomponent.green', '"BLUE_IN"'),
    ('set_image_connector_colorcomponent_red', {'connector': 'displayport1', 'value': 'BLUE_IN'}, 'image.connector.displayport1.colorcomponent.red', '"BLUE_IN"'),
    ('set_image_connector_interleaved_bottom_colorcomponent_blue', {'connector': 'displayport1', 'value': 'BLUE_IN'}, 'image.connector.displayport1.interleaved.bottom.colorcomponent.blue', '"BLUE_IN"'),
    ('set_image_connector_interleaved_bottom_colorcomponent_green', {'connector': 'displayport1', 'value': 'BLUE_IN'}, 'image.connector.displayport1.interleaved.bottom.colorcomponent.green', '"BLUE_IN"'),
    ('set_image_connector_interleaved_bottom_colorcomponent_red', {'connector': 'displayport1', 'value': 'BLUE_IN'}, 'image.connector.displayport1.interleaved.bottom.colorcomponent.red', '"BLUE_IN"'),
    ('set_image_connector_interleaved_bottom_override_brightness', {'connector': 'displayport1', 'value': 0.0}, 'image.connector.displayport1.interleaved.bottom.override.brightness', '0.00'),
    ('set_image_connector_interleaved_bottom_override_contrast', {'connector': 'displayport1', 'value': 1.0}, 'image.connector.displayport1.interleaved.bottom.override.contrast', '1.00'),
    ('set_image_connector_interleaved_bottom_override_enable', {'connector': 'displayport1', 'value': True}, 'image.connector.displayport1.interleaved.bottom.override.enable', 'true'),
    ('set_image_connector_interleaved_bottom_override_gamma', {'connector': 'displayport1', 'value': 2.0}, 'image.connector.displayport1.interleaved.bottom.override.gamma', '2.00'),
    ('set_image_connector_interleaved_bottom_override_gammatype', {'connector': 'displayport1', 'value': 'Example 1'}, 'image.connector.displayport1.interleaved.bottom.override.gammatype', '"Example 1"'),
    ('set_image_connector_interleaved_bottom_override_saturation', {'connector': 'displayport1', 'value': 1.0}, 'image.connector.displayport1.interleaved.bottom.override.saturation', '1.00'),
    ('set_image_connector_interleaved_top_colorcomponent_blue', {'connector': 'displayport1', 'value': 'BLUE_IN'}, 'image.connector.displayport1.interleaved.top.colorcomponent.blue', '"BLUE_IN"'),
    ('set_image_connector_interleaved_top_colorcomponent_green', {'connector': 'displayport1', 'value': 'BLUE_IN'}, 'image.connector.displayport1.interleaved.top.colorcomponent.green', '"BLUE_IN"'),
    ('set_image_connector_interleaved_top_colorcomponent_red', {'connector': 'displayport1', 'value': 'BLUE_IN'}, 'image.connector.displayport1.interleaved.top.colorcomponent.red', '"BLUE_IN"'),
    ('set_image_connector_interleaved_top_override_brightness', {'connector': 'displayport1', 'value': 0.0}, 'image.connector.displayport1.interleaved.top.override.brightness', '0.00'),
    ('set_image_connector_interleaved_top_override_contrast', {'connector': 'displayport1', 'value': 1.0}, 'image.connector.displayport1.interleaved.top.override.contrast', '1.00'),
    ('set_image_connector_interleaved_top_override_enable', {'connector': 'displayport1', 'value': True}, 'image.connector.displayport1.interleaved.top.override.enable', 'true'),
    ('set_image_connector_interleaved_top_override_gamma', {'connector': 'displayport1', 'value': 2.0}, 'image.connector.displayport1.interleaved.top.override.gamma', '2.00'),
    ('set_image_connector_interleaved_top_override_gammatype', {'connector': 'displayport1', 'value': 'Example 1'}, 'image.connector.displayport1.interleaved.top.override.gammatype', '"Example 1"'),
    ('set_image_connector_interleaved_top_override_saturation', {'connector': 'displayport1', 'value': 1.0}, 'image.connector.displayport1.interleaved.top.override.saturation', '1.00'),
    ('set_image_connector_override_brightness', {'connector': 'displayport1', 'value': 0.0}, 'image.connector.displayport1.override.brightness', '0.00'),
    ('set_image_connector_override_contrast', {'connector': 'displayport1', 'value': 1.0}, 'image.connector.displayport1.override.contrast', '1.00'),
    ('set_image_connector_override_enable', {'connector': 'displayport1', 'value': True}, 'image.connector.displayport1.override.enable', 'true'),
    ('set_image_connector_override_gamma', {'connector': 'displayport1', 'value': 2.0}, 'image.connector.displayport1.override.gamma', '2.00'),
    ('set_image_connector_override_gammatype', {'connector': 'displayport1', 'value': 'Example 1'}, 'image.connector.displayport1.override.gammatype', '"Example 1"'),
    ('set_image_connector_override_saturation', {'connector': 'displayport1', 'value': 1.0}, 'image.connector.displayport1.override.saturation', '1.00'),
    ('set_image_contrast', {'value': 1.0}, 'image.contrast', '1.00'),
    ('set_image_convergence_blue', {'value': {'x': 10, 'y': 20}}, 'image.convergence.blue', '{"x":10,"y":20}'),
    ('set_image_convergence_green', {'value': {'x': 10, 'y': 20}}, 'image.convergence.green', '{"x":10,"y":20}'),
    ('set_image_convergence_red', {'value': {'x': 10, 'y': 20}}, 'image.convergence.red', '{"x":10,"y":20}'),
    ('set_image_display_advanceddisplaymodeoverride', {'value': 'Example 1'}, 'image.display.advanceddisplaymodeoverride', '"Example 1"'),
    ('set_image_display_desireddisplaymode', {'value': 'IGPixelShiftFullNV'}, 'image.display.desireddisplaymode', '"IGPixelShiftFullNV"'),
    ('set_image_display_enableadvanceddisplaymodeoverride', {'value': True}, 'image.display.enableadvanceddisplaymodeoverride', 'true'),
    ('set_image_display_extenddisplaylifetime', {'value': True}, 'image.display.extenddisplaylifetime', 'true'),
    ('set_image_dynamiccontrast_level', {'value': 'HIGH'}, 'image.dynamiccontrast.level', '"HIGH"'),
    ('set_image_gamma', {'value': 2.0}, 'image.gamma', '2.00'),
    ('set_image_gammatype', {'value': 'Example 1'}, 'image.gammatype', '"Example 1"'),
    ('set_image_intensity', {'value': 0.5}, 'image.intensity', '0.50'),
    ('set_image_nightvision_swapchannels', {'value': True}, 'image.nightvision.swapchannels', 'true'),
    ('set_image_orientation', {'value': 'CEILING_REAR'}, 'image.orientation', '"CEILING_REAR"'),
    ('set_image_orientationmode', {'value': 'AUTO_REAR'}, 'image.orientationmode', '"AUTO_REAR"'),
    ('set_image_processing_blacklevel_basicblacklevel_bottom', {'value': 1}, 'image.processing.blacklevel.basicblacklevel.bottom', '1'),
    ('set_image_processing_blacklevel_basicblacklevel_enable', {'value': True}, 'image.processing.blacklevel.basicblacklevel.enable', 'true'),
    ('set_image_processing_blacklevel_basicblacklevel_left', {'value': 1}, 'image.processing.blacklevel.basicblacklevel.left', '1'),
    ('set_image_processing_blacklevel_basicblacklevel_level', {'value': 1}, 'image.processing.blacklevel.basicblacklevel.level', '1'),
    ('set_image_processing_blacklevel_basicblacklevel_right', {'value': 1}, 'image.processing.blacklevel.basicblacklevel.right', '1'),
    ('set_image_processing_blacklevel_basicblacklevel_top', {'value': 1}, 'image.processing.blacklevel.basicblacklevel.top', '1'),
    ('set_image_processing_blacklevel_bluegain', {'value': 0.5}, 'image.processing.blacklevel.bluegain', '0.50'),
    ('set_image_processing_blacklevel_file_enable', {'value': True}, 'image.processing.blacklevel.file.enable', 'true'),
    ('set_image_processing_blacklevel_file_selected', {'value': 'Example 1'}, 'image.processing.blacklevel.file.selected', '"Example 1"'),
    ('set_image_processing_blacklevel_greengain', {'value': 0.5}, 'image.processing.blacklevel.greengain', '0.50'),
    ('set_image_processing_blacklevel_linkrgb', {'value': True}, 'image.processing.blacklevel.linkrgb', 'true'),
    ('set_image_processing_blacklevel_redgain', {'value': 0.5}, 'image.processing.blacklevel.redgain', '0.50'),
    ('set_image_processing_blend_basicblend_bottom', {'value': {'x': 10, 'y': 20}}, 'image.processing.blend.basicblend.bottom', '{"x":10,"y":20}'),
    ('set_image_processing_blend_basicblend_enable', {'value': True}, 'image.processing.blend.basicblend.enable', 'true'),
    ('set_image_processing_blend_basicblend_left', {'value': {'x': 10, 'y': 20}}, 'image.processing.blend.basicblend.left', '{"x":10,"y":20}'),
    ('set_image_processing_blend_basicblend_right', {'value': {'x': 10, 'y': 20}}, 'image.processing.blend.basicblend.right', '{"x":10,"y":20}'),
    ('set_image_processing_blend_basicblend_top', {'value': {'x': 10, 'y': 20}}, 'image.processing.blend.basicblend.top', '{"x":10,"y":20}'),
    ('set_image_processing_blend_file_enable', {'value': True}, 'image.processing.blend.file.enable', 'true'),
    ('set_image_processing_blend_file_selected', {'value': ['a', 'b']}, 'image.processing.blend.file.selected', '["a","b"]'),
    ('set_image_processing_blend_scurve', {'value': 2.5}, 'image.processing.blend.scurve', '2.50'),
    ('set_image_processing_tiltedbrightness_enable', {'value': True}, 'image.processing.tiltedbrightness.enable', 'true'),
    ('set_image_processing_tiltedbrightness_horizontalcorrection', {'value': -109}, 'image.processing.tiltedbrightness.horizontalcorrection', '-109'),
    ('set_image_processing_tiltedbrightness_verticalcorrection', {'value': -109}, 'image.processing.tiltedbrightness.verticalcorrection', '-109'),
    ('set_image_processing_transportdelay_desired', {'value': 1}, 'image.processing.transportdelay.desired', '1'),
    ('set_image_processing_warp_bow_bottomleftu', {'value': {'x': 10, 'y': 20}}, 'image.processing.warp.bow.bottomleftu', '{"x":10,"y":20}'),
    ('set_image_processing_warp_bow_bottomleftv', {'value': {'x': 10, 'y': 20}}, 'image.processing.warp.bow.bottomleftv', '{"x":10,"y":20}'),
    ('set_image_processing_warp_bow_bottomrightu', {'value': {'x': 10, 'y': 20}}, 'image.processing.warp.bow.bottomrightu', '{"x":10,"y":20}'),
    ('set_image_processing_warp_bow_bottomrightv', {'value': {'x': 10, 'y': 20}}, 'image.processing.warp.bow.bottomrightv', '{"x":10,"y":20}'),
    ('set_image_processing_warp_bow_enable', {'value': True}, 'image.processing.warp.bow.enable', 'true'),
    ('set_image_processing_warp_bow_symmetric', {'value': True}, 'image.processing.warp.bow.symmetric', 'true'),
    ('set_image_processing_warp_bow_topleftu', {'value': {'x': 10, 'y': 20}}, 'image.processing.warp.bow.topleftu', '{"x":10,"y":20}'),
    ('set_image_processing_warp_bow_topleftv', {'value': {'x': 10, 'y': 20}}, 'image.processing.warp.bow.topleftv', '{"x":10,"y":20}'),
    ('set_image_processing_warp_bow_toprightu', {'value': {'x': 10, 'y': 20}}, 'image.processing.warp.bow.toprightu', '{"x":10,"y":20}'),
    ('set_image_processing_warp_bow_toprightv', {'value': {'x': 10, 'y': 20}}, 'image.processing.warp.bow.toprightv', '{"x":10,"y":20}'),
    ('set_image_processing_warp_enable', {'value': True}, 'image.processing.warp.enable', 'true'),
    ('set_image_processing_warp_file_enable', {'value': True}, 'image.processing.warp.file.enable', 'true'),
    ('set_image_processing_warp_file_selected', {'value': 'Example 1'}, 'image.processing.warp.file.selected', '"Example 1"'),
    ('set_image_processing_warp_fourcorners_bottomleft', {'value': {'x': 10, 'y': 20}}, 'image.processing.warp.fourcorners.bottomleft', '{"x":10,"y":20}'),
    ('set_image_processing_warp_fourcorners_bottomright', {'value': {'x': 10, 'y': 20}}, 'image.processing.warp.fourcorners.bottomright', '{"x":10,"y":20}'),
    ('set_image_processing_warp_fourcorners_enable', {'value': True}, 'image.processing.warp.fourcorners.enable', 'true'),
    ('set_image_processing_warp_fourcorners_screenheight', {'value': 0.5}, 'image.processing.warp.fourcorners.screenheight', '0.50'),
    ('set_image_processing_warp_fourcorners_screenwidth', {'value': 0.5}, 'image.processing.warp.fourcorners.screenwidth', '0.50'),
    ('set_image_processing_warp_fourcorners_topleft', {'value': {'x': 10, 'y': 20}}, 'image.processing.warp.fourcorners.topleft', '{"x":10,"y":20}'),
    ('set_image_processing_warp_fourcorners_topright', {'value': {'x': 10, 'y': 20}}, 'image.processing.warp.fourcorners.topright', '{"x":10,"y":20}'),
    ('set_image_processing_warp_position_enable', {'value': True}, 'image.processing.warp.position.enable', 'true'),
    ('set_image_processing_warp_position_horizontal', {'value': 1}, 'image.processing.warp.position.horizontal', '1'),
    ('set_image_processing_warp_position_vertical', {'value': 1}, 'image.processing.warp.position.vertical', '1'),
    ('set_image_processing_warp_zoom_enable', {'value': True}, 'image.processing.warp.zoom.enable', 'true'),
    ('set_image_processing_warp_zoom_factor', {'value': 1.25}, 'image.processing.warp.zoom.factor', '1.25'),
    ('set_image_resolution_resolution', {'value': 'Example 1'}, 'image.resolution.resolution', '"Example 1"'),
    ('set_image_saturation', {'value': 1.0}, 'image.saturation', '1.00'),
    ('set_image_sharpness', {'value': -1}, 'image.sharpness', '-1'),
    ('set_image_smearreduction_mode', {'value': 'Example 1'}, 'image.smearreduction.mode', '"Example 1"'),
    ('set_image_stereo_darktime', {'value': 1}, 'image.stereo.darktime', '1'),
    ('set_image_stereo_glassync_delay', {'value': 1}, 'image.stereo.glassync.delay', '1'),
    ('set_image_stereo_glassync_invert', {'value': True}, 'image.stereo.glassync.invert', 'true'),
    ('set_image_stereo_stereoglasses', {'value': 'Laser6pGlasses'}, 'image.stereo.stereoglasses', '"Laser6pGlasses"'),
    ('set_image_stereo_swapframepair', {'value': True}, 'image.stereo.swapframepair', 'true'),
    ('set_image_testpattern_nosignal_pattern', {'value': 'Example 1'}, 'image.testpattern.nosignal.pattern', '"Example 1"'),
    ('set_image_testpattern_selected', {'value': 'Example 1'}, 'image.testpattern.selected', '"Example 1"'),
    ('set_image_testpattern_show', {'value': True}, 'image.testpattern.show', 'true'),
    ('set_image_window_main_scalingmode', {'value': 'Stretch'}, 'image.window.main.scalingmode', '"Stretch"'),
    ('set_image_window_main_source', {'value': 'Example 1'}, 'image.window.main.source', '"Example 1"'),
    ('set_iot_cloudservices_enable', {'value': True}, 'iot.cloudservices.enable', 'true'),
    ('set_iot_proxy_address', {'value': 'Example 1'}, 'iot.proxy.address', '"Example 1"'),
    ('set_iot_proxy_enabled', {'value': True}, 'iot.proxy.enabled', 'true'),
    ('set_iot_proxy_port', {'value': 1}, 'iot.proxy.port', '1'),
    ('set_iot_proxy_username', {'value': 'Example 1'}, 'iot.proxy.username', '"Example 1"'),
    ('set_iot_termagreement', {'value': True}, 'iot.termagreement', 'true'),
    ('set_network_device_configuration', {'device': 'hdbaset', 'value': 'MANUAL'}, 'network.device.hdbaset.configuration', '"MANUAL"'),
    ('set_network_device_ip4configmanual', {'device': 'hdbaset', 'value': {'x': 10, 'y': 20}}, 'network.device.hdbaset.ip4configmanual', '{"x":10,"y":20}'),
    ('set_network_device_ip6configmanual', {'device': 'hdbaset', 'value': {'x': 10, 'y': 20}}, 'network.device.hdbaset.ip6configmanual', '{"x":10,"y":20}'),
    ('set_network_device_apn', {'device': 'mobile', 'value': 'Example 1'}, 'network.device.mobile.apn', '"Example 1"'),
    ('set_network_device_enable', {'device': 'wifi', 'value': True}, 'network.device.wifi.enable', 'true'),
    ('set_network_domain', {'value': 'Example 1'}, 'network.domain', '"Example 1"'),
    ('set_network_hostname', {'value': 'Example 1'}, 'network.hostname', '"Example 1"'),
    ('set_optics_autofocus_enabled', {'value': True}, 'optics.autofocus.enabled', 'true'),
    ('set_optics_autofocus_integrationinterval', {'value': 1}, 'optics.autofocus.integrationinterval', '1'),
    ('set_optics_autofocus_maxfocus', {'value': 0.5}, 'optics.autofocus.maxfocus', '0.50'),
    ('set_optics_autofocus_maxzoom', {'value': 0.5}, 'optics.autofocus.maxzoom', '0.50'),
    ('set_optics_autofocus_minfocus', {'value': 0.5}, 'optics.autofocus.minfocus', '0.50'),
    ('set_optics_autofocus_minzoom', {'value': 0.5}, 'optics.autofocus.minzoom', '0.50'),
    ('set_optics_focus_enabled', {'value': True}, 'optics.focus.enabled', 'true'),
    ('set_optics_focus_target', {'value': 1}, 'optics.focus.target', '1'),
    ('set_optics_highcontrast_mode', {'value': 'Example 1'}, 'optics.highcontrast.mode', '"Example 1"'),
    ('set_optics_iris_illumination_enabled', {'value': True}, 'optics.iris.illumination.enabled', 'true'),
    ('set_optics_iris_illumination_target', {'value': 1}, 'optics.iris.illumination.target', '1'),
    ('set_optics_iris_lens_enabled', {'value': True}, 'optics.iris.lens.enabled', 'true'),
    ('set_optics_irisoperation', {'value': 'Linked'}, 'optics.irisoperation', '"Linked"'),
    ('set_optics_lens_position_selected', {'value': 'Example 1'}, 'optics.lens.position.selected', '"Example 1"'),
    ('set_optics_lensshift_horizontal_enabled', {'value': True}, 'optics.lensshift.horizontal.enabled', 'true'),
    ('set_optics_lensshift_horizontal_target', {'value': 1}, 'optics.lensshift.horizontal.target', '1'),
    ('set_optics_lensshift_vertical_enabled', {'value': True}, 'optics.lensshift.vertical.enabled', 'true'),
    ('set_optics_lensshift_vertical_target', {'value': 1}, 'optics.lensshift.vertical.target', '1'),
    ('set_optics_shutter_enabled', {'value': True}, 'optics.shutter.enabled', 'true'),
    ('set_optics_shutter_target', {'value': 'Closed'}, 'optics.shutter.target', '"Closed"'),
    ('set_optics_zoom_enabled', {'value': True}, 'optics.zoom.enabled', 'true'),
    ('set_optics_zoom_target', {'value': 1}, 'optics.zoom.target', '1'),
    ('set_peripheral_camera_exposuremode', {'value': 'Auto'}, 'peripheral.camera.exposuremode', '"Auto"'),
    ('set_peripheral_camera_exposuretime', {'value': 0.5}, 'peripheral.camera.exposuretime', '0.50'),
    ('set_peripheral_camera_resolution', {'value': 'High'}, 'peripheral.camera.resolution', '"High"'),
    ('set_peripheral_ranging_sampletype', {'value': 'Continuous'}, 'peripheral.ranging.sampletype', '"Continuous"'),
    ('set_peripheral_xlrfront_enable', {'value': True}, 'peripheral.xlrfront.enable', 'true'),
    ('set_peripheral_xlrfront_outputvoltage', {'value': 'V24'}, 'peripheral.xlrfront.outputvoltage', '"V24"'),
    ('set_remotecontrol_address', {'value': 2}, 'remotecontrol.address', '2'),
    ('set_remotecontrol_broadcastaddress', {'value': 1}, 'remotecontrol.broadcastaddress', '1'),
    ('set_remotecontrol_sensors_enable', {'sensor': 'front', 'value': True}, 'remotecontrol.sensors.front.enable', 'true'),
    ('set_scheduler_pause', {'value': True}, 'scheduler.pause', 'true'),
    ('set_scheduler_scheduledactions', {'value': 'Example 1'}, 'scheduler.scheduledactions', '"Example 1"'),
    ('set_screen_hdrboost', {'value': 1.25}, 'screen.hdrboost', '1.25'),
    ('set_screen_luminance', {'value': 5025.0}, 'screen.luminance', '5025.00'),
    ('set_screen_screensize', {'value': {'x': 10, 'y': 20}}, 'screen.screensize', '{"x":10,"y":20}'),
    ('set_statistics_lampremainingruntime_value', {'value': 1}, 'statistics.lampremainingruntime.value', '1'),
    ('set_statistics_lampruntime_value', {'value': 1}, 'statistics.lampruntime.value', '1'),
    ('set_statistics_lampstrikes_value', {'value': 1}, 'statistics.lampstrikes.value', '1'),
    ('set_statistics_laserruntime_value', {'value': 1}, 'statistics.laserruntime.value', '1'),
    ('set_statistics_maintenance_colorwheel_cleanup_value', {'value': 1}, 'statistics.maintenance.colorwheel.cleanup.value', '1'),
    ('set_statistics_maintenance_colorwheel_phosphorleft_value', {'value': 1}, 'statistics.maintenance.colorwheel.phosphorleft.value', '1'),
    ('set_statistics_maintenance_colorwheel_phosphorright_value', {'value': 1}, 'statistics.maintenance.colorwheel.phosphorright.value', '1'),
    ('set_statistics_maintenance_fan_ar1_value', {'value': 1}, 'statistics.maintenance.fan.ar1.value', '1'),
    ('set_statistics_maintenance_fan_ar2_value', {'value': 1}, 'statistics.maintenance.fan.ar2.value', '1'),
    ('set_statistics_maintenance_fan_ar3_value', {'value': 1}, 'statistics.maintenance.fan.ar3.value', '1'),
    ('set_statistics_maintenance_fan_ar4_value', {'value': 1}, 'statistics.maintenance.fan.ar4.value', '1'),
    ('set_statistics_maintenance_fan_ar5_value', {'value': 1}, 'statistics.maintenance.fan.ar5.value', '1'),
    ('set_statistics_maintenance_fan_driver_value', {'value': 1}, 'statistics.maintenance.fan.driver.value', '1'),
    ('set_statistics_maintenance_fan_optics_value', {'value': 1}, 'statistics.maintenance.fan.optics.value', '1'),
    ('set_statistics_maintenance_fan_pcb_value', {'value': 1}, 'statistics.maintenance.fan.pcb.value', '1'),
    ('set_statistics_maintenance_fan_phosphorleft_value', {'value': 1}, 'statistics.maintenance.fan.phosphorleft.value', '1'),
    ('set_statistics_maintenance_fan_phosphorright_value', {'value': 1}, 'statistics.maintenance.fan.phosphorright.value', '1'),
    ('set_statistics_maintenance_fan_psu_value', {'value': 1}, 'statistics.maintenance.fan.psu.value', '1'),
    ('set_statistics_maintenance_longfrequency_value', {'value': 1}, 'statistics.maintenance.longfrequency.value', '1'),
    ('set_statistics_maintenance_pumps_value', {'value': 1}, 'statistics.maintenance.pumps.value', '1'),
    ('set_statistics_operating_laseron_value', {'value': 1}, 'statistics.operating.laseron.value', '1'),
    ('set_statistics_operating_lasershut_value', {'value': 1}, 'statistics.operating.lasershut.value', '1'),
    ('set_statistics_operating_uptime_value', {'value': 1}, 'statistics.operating.uptime.value', '1'),
    ('set_statistics_operating_value', {'value': 1}, 'statistics.operating.value', '1'),
    ('set_statistics_projectorruntime_value', {'value': 1}, 'statistics.projectorruntime.value', '1'),
    ('set_statistics_systemtime_value', {'value': 1}, 'statistics.systemtime.value', '1'),
    ('set_statistics_uptime_value', {'value': 1}, 'statistics.uptime.value', '1'),
    ('set_system_eco_enable', {'value': True}, 'system.eco.enable', 'true'),
    ('set_system_error_timeout_duration', {'value': 1}, 'system.error.timeout.duration', '1'),
    ('set_system_error_timeout_enable', {'value': True}, 'system.error.timeout.enable', 'true'),
    ('set_system_initialstate', {'value': 'error'}, 'system.initialstate', '"error"'),
    ('set_system_on_timeout_duration', {'value': 61}, 'system.on.timeout.duration', '61'),
    ('set_system_on_timeout_enable', {'value': True}, 'system.on.timeout.enable', 'true'),
    ('set_system_operationalmodes_mode', {'value': 'highambienttemperature'}, 'system.operationalmodes.mode', '"highambienttemperature"'),
    ('set_system_ready_timeout_duration', {'value': 61}, 'system.ready.timeout.duration', '61'),
    ('set_system_ready_timeout_enable', {'value': True}, 'system.ready.timeout.enable', 'true'),
    ('set_system_standby_enable', {'value': True}, 'system.standby.enable', 'true'),
    ('set_system_standby_timeout_duration', {'value': 61}, 'system.standby.timeout.duration', '61'),
    ('set_system_standby_timeout_enable', {'value': True}, 'system.standby.timeout.enable', 'true'),
    ('set_ui_backlight_state', {'value': 'Auto'}, 'ui.backlight.state', '"Auto"'),
    ('set_ui_backlight_timeout', {'value': 6}, 'ui.backlight.timeout', '6'),
    ('set_ui_keyboardshortcut', {'value': {'x': 10, 'y': 20}}, 'ui.keyboardshortcut', '{"x":10,"y":20}'),
    ('set_ui_language', {'value': 'Example 1'}, 'ui.language', '"Example 1"'),
    ('set_ui_layer_adjustment_enable', {'value': True}, 'ui.layer.adjustment.enable', 'true'),
    ('set_ui_layer_adjustment_icon', {'value': 'Example 1'}, 'ui.layer.adjustment.icon', '"Example 1"'),
    ('set_ui_layer_adjustment_layerposition', {'value': 'Center'}, 'ui.layer.adjustment.layerposition', '"Center"'),
    ('set_ui_layer_adjustment_rangedvalue', {'value': {'x': 10, 'y': 20}}, 'ui.layer.adjustment.rangedvalue', '{"x":10,"y":20}'),
    ('set_ui_layer_adjustment_showleftrightbuttons', {'value': True}, 'ui.layer.adjustment.showleftrightbuttons', 'true'),
    ('set_ui_layer_adjustment_showupdownbuttons', {'value': True}, 'ui.layer.adjustment.showupdownbuttons', 'true'),
    ('set_ui_layer_advancedblend_drawing', {'value': 'Example 1'}, 'ui.layer.advancedblend.drawing', '"Example 1"'),
    ('set_ui_layer_advancedblend_enable', {'value': True}, 'ui.layer.advancedblend.enable', 'true'),
    ('set_ui_layer_advancedblend_palette', {'value': ['a', 'b']}, 'ui.layer.advancedblend.palette', '["a","b"]'),
    ('set_ui_layer_basicblacklevel_color', {'value': 'Example 1'}, 'ui.layer.basicblacklevel.color', '"Example 1"'),
    ('set_ui_layer_basicblacklevel_enable', {'value': True}, 'ui.layer.basicblacklevel.enable', 'true'),
    ('set_ui_layer_basicblacklevel_selection', {'value': {'x': 10, 'y': 20}}, 'ui.layer.basicblacklevel.selection', '{"x":10,"y":20}'),
    ('set_ui_layer_basicblacklevel_selectioncolor', {'value': 'Example 1'}, 'ui.layer.basicblacklevel.selectioncolor', '"Example 1"'),
    ('set_ui_layer_basicblend_color', {'value': 'Example 1'}, 'ui.layer.basicblend.color', '"Example 1"'),
    ('set_ui_layer_basicblend_enable', {'value': True}, 'ui.layer.basicblend.enable', 'true'),
    ('set_ui_layer_basicblend_selection', {'value': {'x': 10, 'y': 20}}, 'ui.layer.basicblend.selection', '{"x":10,"y":20}'),
    ('set_ui_layer_basicblend_selectioncolor', {'value': 'Example 1'}, 'ui.layer.basicblend.selectioncolor', '"Example 1"'),
    ('set_ui_layer_blank_border', {'value': True}, 'ui.layer.blank.border', 'true'),
    ('set_ui_layer_blank_bordercolor', {'value': 'Example 1'}, 'ui.layer.blank.bordercolor', '"Example 1"'),
    ('set_ui_layer_blank_enable', {'value': True}, 'ui.layer.blank.enable', 'true'),
    ('set_ui_layer_blank_icon', {'value': 'Example 1'}, 'ui.layer.blank.icon', '"Example 1"'),
    ('set_ui_layer_blank_show', {'value': True}, 'ui.layer.blank.show', 'true'),
    ('set_ui_layer_blank_showtext', {'value': True}, 'ui.layer.blank.showtext', 'true'),
    ('set_ui_layer_blank_text', {'value': 'Example 1'}, 'ui.layer.blank.text', '"Example 1"'),
    ('set_ui_layer_fourcorner_cornercolor', {'value': 'Example 1'}, 'ui.layer.fourcorner.cornercolor', '"Example 1"'),
    ('set_ui_layer_fourcorner_enable', {'value': True}, 'ui.layer.fourcorner.enable', 'true'),
    ('set_ui_layer_fourcorner_linecolor', {'value': 'Example 1'}, 'ui.layer.fourcorner.linecolor', '"Example 1"'),
    ('set_ui_layer_fourcorner_lines', {'value': True}, 'ui.layer.fourcorner.lines', 'true'),
    ('set_ui_layer_fourcorner_selection', {'value': {'x': 10, 'y': 20}}, 'ui.layer.fourcorner.selection', '{"x":10,"y":20}'),
    ('set_ui_layer_fourcorner_selectioncolor', {'value': 'Example 1'}, 'ui.layer.fourcorner.selectioncolor', '"Example 1"'),
    ('set_ui_layer_grid_color', {'value': 'Example 1'}, 'ui.layer.grid.color', '"Example 1"'),
    ('set_ui_layer_grid_enable', {'value': True}, 'ui.layer.grid.enable', 'true'),
    ('set_ui_layer_grid_lines', {'value': True}, 'ui.layer.grid.lines', 'true'),
    ('set_ui_layer_grid_mark', {'value': ['a', 'b']}, 'ui.layer.grid.mark', '["a","b"]'),
    ('set_ui_layer_grid_points', {'value': '33x33'}, 'ui.layer.grid.points', '"33x33"'),
    ('set_ui_layer_grid_showlines', {'value': True}, 'ui.layer.grid.showlines', 'true'),
    ('set_ui_layer_grid_showpoints', {'value': True}, 'ui.layer.grid.showpoints', 'true'),
    ('set_ui_length', {'value': 'IN'}, 'ui.length', '"IN"'),
    ('set_ui_lensmenu', {'value': True}, 'ui.lensmenu', 'true'),
    ('set_ui_luminance', {'value': 'FL'}, 'ui.luminance', '"FL"'),
    ('set_ui_menu', {'value': True}, 'ui.menu', 'true'),
    ('set_ui_menuposition', {'value': 'Center'}, 'ui.menuposition', '"Center"'),
    ('set_ui_messageposition', {'value': 'Center'}, 'ui.messageposition', '"Center"'),
    ('set_ui_minimize', {'value': True}, 'ui.minimize', 'true'),
    ('set_ui_notificationfiltercodes', {'value': ['a', 'b']}, 'ui.notificationfiltercodes', '["a","b"]'),
    ('set_ui_notificationfilterseverity', {'value': 'NONE'}, 'ui.notificationfilterseverity', '"NONE"'),
    ('set_ui_osd', {'value': True}, 'ui.osd', 'true'),
    ('set_ui_patternmenu', {'value': True}, 'ui.patternmenu', 'true'),
    ('set_ui_poweroffhint', {'value': True}, 'ui.poweroffhint', 'true'),
    ('set_ui_sourcemenu', {'value': True}, 'ui.sourcemenu', 'true'),
    ('set_ui_sourcesignal', {'value': True}, 'ui.sourcesignal', 'true'),
    ('set_ui_sourcesignalposition', {'value': 'Center'}, 'ui.sourcesignalposition', '"Center"'),
    ('set_ui_stealthmode', {'value': 'OnUntilReboot'}, 'ui.stealthmode', '"OnUntilReboot"'),
    ('set_ui_temperature', {'value': 'F'}, 'ui.temperature', '"F"'),
    ('set_ui_theme', {'value': 'Dark'}, 'ui.theme', '"Dark"'),
    ('set_ui_touchscreen_status', {'value': 'Complete'}, 'ui.touchscreen.status', '"Complete"'),
]
for _command, _input, _property, _value in _SETTERS:
    text(BP, _command, _input, '{"jsonrpc":"2.0","method":"property.set","params":{"property":%s,"value":%s},"id":1}'
         % (_json.dumps(_property), _value))

text(BP, 'get_property', {'property': 'image.brightness'}, '{"jsonrpc":"2.0","method":"property.get","params":{"property":"image.brightness"},"id":1}', **{'device_reply': '{"jsonrpc":"2.0","result":0.15,"id":1}', 'expect_result': {'ok': {'kind': 'value', 'value': 0.15}}})
text(BP, 'set_property', {'property': 'image.brightness', 'value': 0.15}, '{"jsonrpc":"2.0","method":"property.set","params":{"property":"image.brightness","value":0.15},"id":1}', **{'device_reply': '{"jsonrpc":"2.0","result":true,"id":1}', 'expect_result': {'ok': {'kind': 'ack'}}})
text(BP, 'call_method', {'method': 'optics.focus.stepforward', 'params': {'steps': 10}}, '{"jsonrpc":"2.0","method":"optics.focus.stepforward","params":{"steps":10},"id":1}', **{'device_reply': '{"jsonrpc":"2.0","result":null,"id":1}', 'expect_result': {'ok': {'kind': 'value', 'value': None}}})
text(BP, 'subscribe_property', {'property': 'image.contrast'}, '{"jsonrpc":"2.0","method":"property.subscribe","params":{"property":"image.contrast"},"id":1}')
text(BP, 'unsubscribe_property', {'property': 'image.contrast'}, '{"jsonrpc":"2.0","method":"property.unsubscribe","params":{"property":"image.contrast"},"id":1}')
text(BP, 'subscribe_signal', {'signal': 'image.processing.warpgrid.gridchanged'}, '{"jsonrpc":"2.0","method":"signal.subscribe","params":{"signal":"image.processing.warpgrid.gridchanged"},"id":1}')
text(BP, 'unsubscribe_signal', {'signal': 'modelupdated'}, '{"jsonrpc":"2.0","method":"signal.unsubscribe","params":{"signal":"modelupdated"},"id":1}')
text(BP, 'introspect', {'object': 'illumination.sources', 'recursive': False}, '{"jsonrpc":"2.0","method":"introspect","params":{"object":"illumination.sources","recursive":false},"id":1}')
text(BP, 'authenticate', {'code': 98765}, '{"jsonrpc":"2.0","method":"authenticate","params":{"code":98765},"id":1}', **{'device_reply': '{"jsonrpc":"2.0","result":true,"id":1}', 'expect_result': {'ok': {'kind': 'value', 'value': True}}})
text(BP, 'power_on', {}, '{"jsonrpc":"2.0","method":"system.poweron","id":1}')
text(BP, 'power_off', {}, '{"jsonrpc":"2.0","method":"system.poweroff","id":1}')
text(BP, 'go_to_ready', {}, '{"jsonrpc":"2.0","method":"system.gotoready","id":1}')
text(BP, 'go_to_eco', {}, '{"jsonrpc":"2.0","method":"system.gotoeco","id":1}')
text(BP, 'signal_activity', {}, '{"jsonrpc":"2.0","method":"system.activity","id":1}')
text(BP, 'list_reset_domains', {}, '{"jsonrpc":"2.0","method":"system.listresetdomains","id":1}')
text(BP, 'reset_domains', {'domains': ['ImageWarp', 'ImageBlend']}, '{"jsonrpc":"2.0","method":"system.reset","params":{"domains":["ImageWarp","ImageBlend"]},"id":1}')
text(BP, 'reset_all', {}, '{"jsonrpc":"2.0","method":"system.resetall","id":1}')
text(BP, 'get_identifications', {}, '{"jsonrpc":"2.0","method":"system.getidentifications","id":1}')
text(BP, 'get_identification', {'identification': 'SerialNumber'}, '{"jsonrpc":"2.0","method":"system.getidentification","params":{"identification":"SerialNumber"},"id":1}')
text(BP, 'get_system_date', {}, '{"jsonrpc":"2.0","method":"system.getsystemdate","id":1}')
text(BP, 'get_local_date', {}, '{"jsonrpc":"2.0","method":"system.date.getlocaldate","id":1}')
text(BP, 'get_utc_date', {}, '{"jsonrpc":"2.0","method":"system.date.getsystemdate","id":1}')
text(BP, 'list_boards', {}, '{"jsonrpc":"2.0","method":"system.boards.getboardlist","id":1}')
text(BP, 'list_missing_boards', {}, '{"jsonrpc":"2.0","method":"system.boards.getmissingboardlist","id":1}')
text(BP, 'get_board_info', {'board': 'mainboard'}, '{"jsonrpc":"2.0","method":"system.boards.getboardinfo","params":{"boardname":"mainboard"},"id":1}')
text(BP, 'get_board_module_info', {'board': 'mainboard'}, '{"jsonrpc":"2.0","method":"system.boards.getmoduleinfo","params":{"boardname":"mainboard"},"id":1}')
text(BP, 'save_license_registration_to_usb', {}, '{"jsonrpc":"2.0","method":"system.license.file.saveregistrationtousb","id":1}')
text(BP, 'upload_license_from_usb', {}, '{"jsonrpc":"2.0","method":"system.license.file.uploadfromusb","id":1}')
text(BP, 'get_laser_serial_number', {}, '{"jsonrpc":"2.0","method":"illumination.laser.getserialnumber","id":1}')
text(BP, 'open_shutter', {}, '{"jsonrpc":"2.0","method":"property.set","params":{"property":"optics.shutter.target","value":"Open"},"id":1}', **{'device_reply': '{"jsonrpc":"2.0","result":true,"id":1}', 'expect_result': {'ok': {'kind': 'ack'}}})
text(BP, 'close_shutter', {}, '{"jsonrpc":"2.0","method":"property.set","params":{"property":"optics.shutter.target","value":"Closed"},"id":1}', **{'device_reply': '{"jsonrpc":"2.0","error":{"code":-32601,"message":"Property not found"},"id":1}', 'expect_result': {'error': {'error': 'device_error'}}})
text(BP, 'toggle_shutter', {}, '{"jsonrpc":"2.0","method":"optics.shutter.toggle","id":1}')
text(BP, 'get_shutter_state', {}, '{"jsonrpc":"2.0","method":"optics.shutter.getstate","id":1}')
text(BP, 'calibrate_shutter', {}, '{"jsonrpc":"2.0","method":"optics.shutter.calibrate","id":1}')
text(BP, 'set_source', {'source': 'DisplayPort 1'}, '{"jsonrpc":"2.0","method":"property.set","params":{"property":"image.window.main.source","value":"DisplayPort 1"},"id":1}')
text(BP, 'show_test_pattern', {'pattern': 'Crosshatch'}, ['{"jsonrpc":"2.0","method":"property.set","params":{"property":"image.testpattern.selected","value":"Crosshatch"},"id":1}', '{"jsonrpc":"2.0","method":"property.set","params":{"property":"image.testpattern.show","value":true},"id":1}'], **{'device_reply': '{"jsonrpc":"2.0","result":true,"id":1}{"jsonrpc":"2.0","result":true,"id":1}', 'expect_result': {'ok': {'kind': 'ack'}}})
text(BP, 'hide_test_pattern', {}, '{"jsonrpc":"2.0","method":"property.set","params":{"property":"image.testpattern.show","value":false},"id":1}')
text(BP, 'set_laser_power', {'power': 40.0}, '{"jsonrpc":"2.0","method":"property.set","params":{"property":"illumination.sources.laser.power","value":40.00},"id":1}')
text(BP, 'zoom_step_forward', {'steps': 10}, '{"jsonrpc":"2.0","method":"optics.zoom.stepforward","params":{"steps":10},"id":1}')
text(BP, 'zoom_step_reverse', {'steps': 10}, '{"jsonrpc":"2.0","method":"optics.zoom.stepreverse","params":{"steps":10},"id":1}')
text(BP, 'zoom_run_forward', {}, '{"jsonrpc":"2.0","method":"optics.zoom.runforward","id":1}')
text(BP, 'zoom_run_reverse', {}, '{"jsonrpc":"2.0","method":"optics.zoom.runreverse","id":1}')
text(BP, 'zoom_run_forward_time', {'milliseconds': 500}, '{"jsonrpc":"2.0","method":"optics.zoom.runforwardtime","params":{"milliseconds":500},"id":1}')
text(BP, 'zoom_run_reverse_time', {'milliseconds': 500}, '{"jsonrpc":"2.0","method":"optics.zoom.runreversetime","params":{"milliseconds":500},"id":1}')
text(BP, 'zoom_stop', {}, '{"jsonrpc":"2.0","method":"optics.zoom.stop","id":1}')
text(BP, 'zoom_calibrate', {}, '{"jsonrpc":"2.0","method":"optics.zoom.calibrate","id":1}')
text(BP, 'zoom_add_location', {'location': 'Wide'}, '{"jsonrpc":"2.0","method":"optics.zoom.addlocation","params":{"location":"Wide"},"id":1}')
text(BP, 'zoom_go_to_location', {'location': 'Wide'}, '{"jsonrpc":"2.0","method":"optics.zoom.setlocation","params":{"location":"Wide"},"id":1}')
text(BP, 'focus_step_forward', {'steps': 10}, '{"jsonrpc":"2.0","method":"optics.focus.stepforward","params":{"steps":10},"id":1}')
text(BP, 'focus_step_reverse', {'steps': 10}, '{"jsonrpc":"2.0","method":"optics.focus.stepreverse","params":{"steps":10},"id":1}')
text(BP, 'focus_run_forward', {}, '{"jsonrpc":"2.0","method":"optics.focus.runforward","id":1}')
text(BP, 'focus_run_reverse', {}, '{"jsonrpc":"2.0","method":"optics.focus.runreverse","id":1}')
text(BP, 'focus_run_forward_time', {'milliseconds': 500}, '{"jsonrpc":"2.0","method":"optics.focus.runforwardtime","params":{"milliseconds":500},"id":1}')
text(BP, 'focus_run_reverse_time', {'milliseconds': 500}, '{"jsonrpc":"2.0","method":"optics.focus.runreversetime","params":{"milliseconds":500},"id":1}')
text(BP, 'focus_stop', {}, '{"jsonrpc":"2.0","method":"optics.focus.stop","id":1}')
text(BP, 'focus_calibrate', {}, '{"jsonrpc":"2.0","method":"optics.focus.calibrate","id":1}')
text(BP, 'focus_add_location', {'location': 'Wide'}, '{"jsonrpc":"2.0","method":"optics.focus.addlocation","params":{"location":"Wide"},"id":1}')
text(BP, 'focus_go_to_location', {'location': 'Wide'}, '{"jsonrpc":"2.0","method":"optics.focus.setlocation","params":{"location":"Wide"},"id":1}')
text(BP, 'shift_horizontal_step_forward', {'steps': 10}, '{"jsonrpc":"2.0","method":"optics.lensshift.horizontal.stepforward","params":{"steps":10},"id":1}')
text(BP, 'shift_horizontal_step_reverse', {'steps': 10}, '{"jsonrpc":"2.0","method":"optics.lensshift.horizontal.stepreverse","params":{"steps":10},"id":1}')
text(BP, 'shift_horizontal_run_forward', {}, '{"jsonrpc":"2.0","method":"optics.lensshift.horizontal.runforward","id":1}')
text(BP, 'shift_horizontal_run_reverse', {}, '{"jsonrpc":"2.0","method":"optics.lensshift.horizontal.runreverse","id":1}')
text(BP, 'shift_horizontal_run_forward_time', {'milliseconds': 500}, '{"jsonrpc":"2.0","method":"optics.lensshift.horizontal.runforwardtime","params":{"milliseconds":500},"id":1}')
text(BP, 'shift_horizontal_run_reverse_time', {'milliseconds': 500}, '{"jsonrpc":"2.0","method":"optics.lensshift.horizontal.runreversetime","params":{"milliseconds":500},"id":1}')
text(BP, 'shift_horizontal_stop', {}, '{"jsonrpc":"2.0","method":"optics.lensshift.horizontal.stop","id":1}')
text(BP, 'shift_horizontal_calibrate', {}, '{"jsonrpc":"2.0","method":"optics.lensshift.horizontal.calibrate","id":1}')
text(BP, 'shift_horizontal_home', {}, '{"jsonrpc":"2.0","method":"optics.lensshift.horizontal.home","id":1}')
text(BP, 'shift_horizontal_home_and_return', {}, '{"jsonrpc":"2.0","method":"optics.lensshift.horizontal.homeandreturn","id":1}')
text(BP, 'shift_horizontal_add_location', {'location': 'Wide'}, '{"jsonrpc":"2.0","method":"optics.lensshift.horizontal.addlocation","params":{"location":"Wide"},"id":1}')
text(BP, 'shift_horizontal_go_to_location', {'location': 'Wide'}, '{"jsonrpc":"2.0","method":"optics.lensshift.horizontal.setlocation","params":{"location":"Wide"},"id":1}')
text(BP, 'shift_horizontal_save_location', {'location': 2}, '{"jsonrpc":"2.0","method":"optics.lensshift.horizontal.savelocation","params":{"location":2},"id":1}')
text(BP, 'shift_horizontal_recall_location', {'location': 2}, '{"jsonrpc":"2.0","method":"optics.lensshift.horizontal.recalllocation","params":{"location":2},"id":1}')
text(BP, 'shift_horizontal_get_locations', {}, '{"jsonrpc":"2.0","method":"optics.lensshift.horizontal.getlocations","id":1}')
text(BP, 'shift_vertical_step_forward', {'steps': 10}, '{"jsonrpc":"2.0","method":"optics.lensshift.vertical.stepforward","params":{"steps":10},"id":1}')
text(BP, 'shift_vertical_step_reverse', {'steps': 10}, '{"jsonrpc":"2.0","method":"optics.lensshift.vertical.stepreverse","params":{"steps":10},"id":1}')
text(BP, 'shift_vertical_run_forward', {}, '{"jsonrpc":"2.0","method":"optics.lensshift.vertical.runforward","id":1}')
text(BP, 'shift_vertical_run_reverse', {}, '{"jsonrpc":"2.0","method":"optics.lensshift.vertical.runreverse","id":1}')
text(BP, 'shift_vertical_run_forward_time', {'milliseconds': 500}, '{"jsonrpc":"2.0","method":"optics.lensshift.vertical.runforwardtime","params":{"milliseconds":500},"id":1}')
text(BP, 'shift_vertical_run_reverse_time', {'milliseconds': 500}, '{"jsonrpc":"2.0","method":"optics.lensshift.vertical.runreversetime","params":{"milliseconds":500},"id":1}')
text(BP, 'shift_vertical_stop', {}, '{"jsonrpc":"2.0","method":"optics.lensshift.vertical.stop","id":1}')
text(BP, 'shift_vertical_calibrate', {}, '{"jsonrpc":"2.0","method":"optics.lensshift.vertical.calibrate","id":1}')
text(BP, 'shift_vertical_home', {}, '{"jsonrpc":"2.0","method":"optics.lensshift.vertical.home","id":1}')
text(BP, 'shift_vertical_home_and_return', {}, '{"jsonrpc":"2.0","method":"optics.lensshift.vertical.homeandreturn","id":1}')
text(BP, 'shift_vertical_add_location', {'location': 'Wide'}, '{"jsonrpc":"2.0","method":"optics.lensshift.vertical.addlocation","params":{"location":"Wide"},"id":1}')
text(BP, 'shift_vertical_go_to_location', {'location': 'Wide'}, '{"jsonrpc":"2.0","method":"optics.lensshift.vertical.setlocation","params":{"location":"Wide"},"id":1}')
text(BP, 'shift_vertical_save_location', {'location': 2}, '{"jsonrpc":"2.0","method":"optics.lensshift.vertical.savelocation","params":{"location":2},"id":1}')
text(BP, 'shift_vertical_recall_location', {'location': 2}, '{"jsonrpc":"2.0","method":"optics.lensshift.vertical.recalllocation","params":{"location":2},"id":1}')
text(BP, 'shift_vertical_get_locations', {}, '{"jsonrpc":"2.0","method":"optics.lensshift.vertical.getlocations","id":1}')
text(BP, 'illumination_iris_step_forward', {'steps': 10}, '{"jsonrpc":"2.0","method":"optics.iris.illumination.stepforward","params":{"steps":10},"id":1}')
text(BP, 'illumination_iris_step_reverse', {'steps': 10}, '{"jsonrpc":"2.0","method":"optics.iris.illumination.stepreverse","params":{"steps":10},"id":1}')
text(BP, 'illumination_iris_run_forward', {}, '{"jsonrpc":"2.0","method":"optics.iris.illumination.runforward","id":1}')
text(BP, 'illumination_iris_run_reverse', {}, '{"jsonrpc":"2.0","method":"optics.iris.illumination.runreverse","id":1}')
text(BP, 'illumination_iris_run_forward_time', {'milliseconds': 500}, '{"jsonrpc":"2.0","method":"optics.iris.illumination.runforwardtime","params":{"milliseconds":500},"id":1}')
text(BP, 'illumination_iris_run_reverse_time', {'milliseconds': 500}, '{"jsonrpc":"2.0","method":"optics.iris.illumination.runreversetime","params":{"milliseconds":500},"id":1}')
text(BP, 'illumination_iris_stop', {}, '{"jsonrpc":"2.0","method":"optics.iris.illumination.stop","id":1}')
text(BP, 'illumination_iris_calibrate', {}, '{"jsonrpc":"2.0","method":"optics.iris.illumination.calibrate","id":1}')
text(BP, 'illumination_iris_home', {}, '{"jsonrpc":"2.0","method":"optics.iris.illumination.home","id":1}')
text(BP, 'illumination_iris_home_and_return', {}, '{"jsonrpc":"2.0","method":"optics.iris.illumination.homeandreturn","id":1}')
text(BP, 'illumination_iris_add_location', {'location': 'Wide'}, '{"jsonrpc":"2.0","method":"optics.iris.illumination.addlocation","params":{"location":"Wide"},"id":1}')
text(BP, 'illumination_iris_go_to_location', {'location': 'Wide'}, '{"jsonrpc":"2.0","method":"optics.iris.illumination.setlocation","params":{"location":"Wide"},"id":1}')
text(BP, 'illumination_iris_save_location', {'location': 2}, '{"jsonrpc":"2.0","method":"optics.iris.illumination.savelocation","params":{"location":2},"id":1}')
text(BP, 'illumination_iris_recall_location', {'location': 2}, '{"jsonrpc":"2.0","method":"optics.iris.illumination.recalllocation","params":{"location":2},"id":1}')
text(BP, 'illumination_iris_get_locations', {}, '{"jsonrpc":"2.0","method":"optics.iris.illumination.getlocations","id":1}')
text(BP, 'lens_iris_step_forward', {'steps': 10}, '{"jsonrpc":"2.0","method":"optics.iris.lens.stepforward","params":{"steps":10},"id":1}')
text(BP, 'lens_iris_step_reverse', {'steps': 10}, '{"jsonrpc":"2.0","method":"optics.iris.lens.stepreverse","params":{"steps":10},"id":1}')
text(BP, 'lens_iris_run_forward', {}, '{"jsonrpc":"2.0","method":"optics.iris.lens.runforward","id":1}')
text(BP, 'lens_iris_run_reverse', {}, '{"jsonrpc":"2.0","method":"optics.iris.lens.runreverse","id":1}')
text(BP, 'lens_iris_run_forward_time', {'milliseconds': 500}, '{"jsonrpc":"2.0","method":"optics.iris.lens.runforwardtime","params":{"milliseconds":500},"id":1}')
text(BP, 'lens_iris_run_reverse_time', {'milliseconds': 500}, '{"jsonrpc":"2.0","method":"optics.iris.lens.runreversetime","params":{"milliseconds":500},"id":1}')
text(BP, 'lens_iris_stop', {}, '{"jsonrpc":"2.0","method":"optics.iris.lens.stop","id":1}')
text(BP, 'shift_to_center', {}, '{"jsonrpc":"2.0","method":"optics.shifttocenter","id":1}')
text(BP, 'list_lenses', {}, '{"jsonrpc":"2.0","method":"optics.lens.list","id":1}')
text(BP, 'get_valid_lens_ids', {}, '{"jsonrpc":"2.0","method":"optics.getvalidlensids","id":1}')
text(BP, 'set_lens_id', {'lens_id': 3, 'power_lens_id': 1}, '{"jsonrpc":"2.0","method":"optics.setlensid","params":{"lensid":3,"powerlensid":1},"id":1}')
text(BP, 'calibrate_autofocus', {}, '{"jsonrpc":"2.0","method":"optics.autofocus.calibration.calibrate","id":1}')
text(BP, 'calibrate_high_contrast', {}, '{"jsonrpc":"2.0","method":"optics.highcontrast.calibrate","id":1}')
text(BP, 'list_high_contrast_modes', {}, '{"jsonrpc":"2.0","method":"optics.highcontrast.getavailablemodes","id":1}')
text(BP, 'get_high_contrast_state', {}, '{"jsonrpc":"2.0","method":"optics.highcontrast.getstate","id":1}')
text(BP, 'engage_clo', {}, '{"jsonrpc":"2.0","method":"illumination.clo.engage","id":1}')
text(BP, 'get_light_output', {}, '{"jsonrpc":"2.0","method":"lightmeasurement.getlightoutput","id":1}')
text(BP, 'list_sources', {}, '{"jsonrpc":"2.0","method":"image.source.list","id":1}')
text(BP, 'list_connectors', {}, '{"jsonrpc":"2.0","method":"image.connector.list","id":1}')
text(BP, 'list_windows', {}, '{"jsonrpc":"2.0","method":"image.window.list","id":1}')
text(BP, 'list_resolutions', {}, '{"jsonrpc":"2.0","method":"image.resolution.list","id":1}')
text(BP, 'list_display_modes', {}, '{"jsonrpc":"2.0","method":"image.display.listdisplaymodes","id":1}')
text(BP, 'list_orientation_modes', {}, '{"jsonrpc":"2.0","method":"image.list","id":1}')
text(BP, 'list_gamma_types', {}, '{"jsonrpc":"2.0","method":"image.listgammatypes","id":1}')
text(BP, 'list_brilliantcolor_modes', {}, '{"jsonrpc":"2.0","method":"image.brilliantcolor.list","id":1}')
text(BP, 'list_smear_reduction_modes', {}, '{"jsonrpc":"2.0","method":"image.smearreduction.list","id":1}')
text(BP, 'list_darktimes', {}, '{"jsonrpc":"2.0","method":"image.stereo.listdarktime","id":1}')
text(BP, 'list_test_patterns', {}, '{"jsonrpc":"2.0","method":"image.testpattern.list","id":1}')
text(BP, 'list_internal_test_patterns', {}, '{"jsonrpc":"2.0","method":"image.testpattern.listinternal","id":1}')
text(BP, 'list_nosignal_patterns', {}, '{"jsonrpc":"2.0","method":"image.testpattern.nosignal.list","id":1}')
text(BP, 'list_test_pattern_files', {}, '{"jsonrpc":"2.0","method":"image.testpattern.file.list","id":1}')
text(BP, 'list_warp_files', {}, '{"jsonrpc":"2.0","method":"image.processing.warp.file.list","id":1}')
text(BP, 'list_blend_files', {}, '{"jsonrpc":"2.0","method":"image.processing.blend.file.list","id":1}')
text(BP, 'list_black_level_files', {}, '{"jsonrpc":"2.0","method":"image.processing.blacklevel.file.list","id":1}')
text(BP, 'get_warp_grid', {}, '{"jsonrpc":"2.0","method":"image.processing.warpgrid.getgrid","id":1}')
text(BP, 'get_warp_grid_size', {}, '{"jsonrpc":"2.0","method":"image.processing.warpgrid.getgridsize","id":1}')
text(BP, 'list_source_connectors', {'source': 'displayport1'}, '{"jsonrpc":"2.0","method":"image.source.displayport1.listconnectors","id":1}')
text(BP, 'list_connector_edids', {'connector': 'hdmi'}, '{"jsonrpc":"2.0","method":"image.connector.hdmi.edid.list","id":1}')
text(BP, 'get_blend_area', {'width': 1920.0, 'height': 1200.0}, '{"jsonrpc":"2.0","method":"image.processing.blend.basicblend.getblendarea","params":{"resolution_width":1920.0,"resolution_height":1200.0},"id":1}')
text(BP, 'get_warped_blend_area', {'width': 1920.0, 'height': 1200.0}, '{"jsonrpc":"2.0","method":"image.processing.blend.basicblend.getwarpedblendarea","params":{"resolution_width":1920.0,"resolution_height":1200.0},"id":1}')
text(BP, 'get_black_level_area', {'width': 1920.0, 'height': 1200.0}, '{"jsonrpc":"2.0","method":"image.processing.blacklevel.basicblacklevel.getblacklevelarea","params":{"resolution_width":1920.0,"resolution_height":1200.0},"id":1}')
text(BP, 'get_warped_black_level_area', {'width': 1920.0, 'height': 1200.0}, '{"jsonrpc":"2.0","method":"image.processing.blacklevel.basicblacklevel.getwarpedblacklevelarea","params":{"resolution_width":1920.0,"resolution_height":1200.0},"id":1}')
text(BP, 'get_scaled_warp_grid', {'resolution': {'x': 1920, 'y': 1200}}, '{"jsonrpc":"2.0","method":"image.processing.warpgrid.getscaledgrid","params":{"resolution":{"x":1920,"y":1200}},"id":1}')
text(BP, 'get_scaled_corners', {'resolution': {'x': 1920, 'y': 1200}}, '{"jsonrpc":"2.0","method":"image.processing.warp.fourcorners.getscaledcorners","params":{"resolution":{"x":1920,"y":1200}},"id":1}')
text(BP, 'warp_scaled_points', {'points': [{'X': 10.0, 'Y': 20.0}], 'resolution': {'X': 1920.0, 'Y': 1200.0}}, '{"jsonrpc":"2.0","method":"image.processing.warp.warpscaledpoints","params":{"points":[{"X":10.0,"Y":20.0}],"resolution":{"X":1920.0,"Y":1200.0}},"id":1}')
text(BP, 'delete_warp_file', {'filename': 'warp.xml'}, '{"jsonrpc":"2.0","method":"image.processing.warp.file.delete","params":{"filename":"warp.xml"},"id":1}')
text(BP, 'delete_blend_file', {'filename': 'warp.xml'}, '{"jsonrpc":"2.0","method":"image.processing.blend.file.delete","params":{"filename":"warp.xml"},"id":1}')
text(BP, 'delete_black_level_file', {'filename': 'warp.xml'}, '{"jsonrpc":"2.0","method":"image.processing.blacklevel.file.delete","params":{"filename":"warp.xml"},"id":1}')
text(BP, 'delete_test_pattern_file', {'filename': 'warp.xml'}, '{"jsonrpc":"2.0","method":"image.testpattern.file.delete","params":{"filename":"warp.xml"},"id":1}')
text(BP, 'copy_color_preset_to_custom', {'preset': 'Rec709'}, '{"jsonrpc":"2.0","method":"image.color.p7.custom.copypresettocustom","params":{"presetname":"Rec709"},"id":1}')
text(BP, 'reset_color_preset', {'preset': 'Rec709'}, '{"jsonrpc":"2.0","method":"image.color.p7.custom.resetpreset","params":{"presetname":"Rec709"},"id":1}')
text(BP, 'reset_color_to_native', {}, '{"jsonrpc":"2.0","method":"image.color.p7.custom.resettonative","id":1}')
text(BP, 'make_color_balanced', {}, '{"jsonrpc":"2.0","method":"image.color.p7.custom.makebalanced","id":1}')
text(BP, 'next_rgb_mode', {}, '{"jsonrpc":"2.0","method":"image.color.rgbmode.nextrgbmode","id":1}')
text(BP, 'toggle_test_pattern', {}, '{"jsonrpc":"2.0","method":"image.testpattern.toggle","id":1}')
text(BP, 'get_test_pattern_properties', {'id': 'Crosshatch'}, '{"jsonrpc":"2.0","method":"image.testpattern.getproperties","params":{"id":"Crosshatch"},"id":1}')
text(BP, 'set_test_pattern_properties', {'id': 'Crosshatch', 'properties': [{'key': 'color', 'value': 'white'}]}, '{"jsonrpc":"2.0","method":"image.testpattern.setproperties","params":{"id":"Crosshatch","properties":[{"key":"color","value":"white"}]},"id":1}')
text(BP, 'next_test_pattern', {'iterator': 'optics'}, '{"jsonrpc":"2.0","method":"image.testpattern.iterator.optics.next","id":1}')
text(BP, 'list_test_pattern_set', {'iterator': 'shift'}, '{"jsonrpc":"2.0","method":"image.testpattern.iterator.shift.listpossiblepatterns","id":1}')
text(BP, 'list_notifications', {}, '{"jsonrpc":"2.0","method":"notification.list","id":1}')
text(BP, 'list_notifications_by_severity', {'severity': 'error'}, '{"jsonrpc":"2.0","method":"notification.error.list","id":1}')
text(BP, 'dismiss_notification', {'id': '42', 'response': 'OK'}, '{"jsonrpc":"2.0","method":"notification.dismiss","params":{"id":"42","response":"OK"},"id":1}')
text(BP, 'suppress_notification', {'code': '1-2014'}, '{"jsonrpc":"2.0","method":"notification.suppress","params":{"code":"1-2014"},"id":1}')
text(BP, 'unsuppress_notification', {'code': '1-2014'}, '{"jsonrpc":"2.0","method":"notification.unsuppress","params":{"code":"1-2014"},"id":1}')
text(BP, 'unsuppress_all_notifications', {}, '{"jsonrpc":"2.0","method":"notification.unsuppressall","id":1}')
text(BP, 'list_suppressed_notifications', {}, '{"jsonrpc":"2.0","method":"notification.listsuppressed","id":1}')
text(BP, 'get_notification_log', {'minimum_severity': 'WARNING', 'start': 0, 'count': 20}, '{"jsonrpc":"2.0","method":"notification.log","params":{"minimumseverity":"WARNING","start":0,"count":20},"id":1}')
text(BP, 'get_control_blocks', {'type': 'Sensor', 'value_type': 'Voltage'}, '{"jsonrpc":"2.0","method":"environment.getcontrolblocks","params":{"type":"Sensor","valuetype":"Voltage"},"id":1}')
text(BP, 'get_alarm_info', {}, '{"jsonrpc":"2.0","method":"environment.getalarminfo","id":1}')
text(BP, 'list_counters', {}, '{"jsonrpc":"2.0","method":"statistics.listcounters","id":1}')
text(BP, 'activate_preset', {'preset': 1}, '{"jsonrpc":"2.0","method":"profile.activatepreset","params":{"preset":1},"id":1}')
text(BP, 'activate_profile', {'name': 'Show'}, '{"jsonrpc":"2.0","method":"profile.activateprofile","params":{"name":"Show"},"id":1}')
text(BP, 'assign_preset', {'preset': 1, 'profile': 'Show'}, '{"jsonrpc":"2.0","method":"profile.assignpreset","params":{"preset":1,"profile":"Show"},"id":1}')
text(BP, 'create_profile', {'name': 'Show', 'domains': ['Image']}, '{"jsonrpc":"2.0","method":"profile.createprofile","params":{"profilename":"Show","domains":["Image"]},"id":1}')
text(BP, 'delete_profile', {'name': 'Show'}, '{"jsonrpc":"2.0","method":"profile.deleteprofile","params":{"name":"Show"},"id":1}')
text(BP, 'get_profile_domains', {'name': 'Show'}, '{"jsonrpc":"2.0","method":"profile.getdomainsforprofile","params":{"name":"Show"},"id":1}')
text(BP, 'get_preset_for_profile', {'name': 'Show'}, '{"jsonrpc":"2.0","method":"profile.presetforprofile","params":{"name":"Show"},"id":1}')
text(BP, 'get_profile_for_preset', {'preset': 1}, '{"jsonrpc":"2.0","method":"profile.profileforpreset","params":{"preset":1},"id":1}')
text(BP, 'press_and_release_key', {'key': 'RC_SHUTTER_CLOSE'}, '{"jsonrpc":"2.0","method":"keydispatcher.sendclickevent","params":{"key":"RC_SHUTTER_CLOSE"},"id":1}')
text(BP, 'press_key', {'key': 'RC_ZOOM_PLUS'}, '{"jsonrpc":"2.0","method":"keydispatcher.sendpressevent","params":{"key":"RC_ZOOM_PLUS"},"id":1}')
text(BP, 'release_key', {'key': 'RC_ZOOM_PLUS'}, '{"jsonrpc":"2.0","method":"keydispatcher.sendreleaseevent","params":{"key":"RC_ZOOM_PLUS"},"id":1}')
text(BP, 'post_key_event', {'event': 'CLICKED', 'key': 'RC_PATTERN'}, '{"jsonrpc":"2.0","method":"keydispatcher.postevent","params":{"eventtype":"CLICKED","key":"RC_PATTERN"},"id":1}')
text(BP, 'led_activity', {}, '{"jsonrpc":"2.0","method":"led.activity","id":1}')
text(BP, 'mute_leds', {}, '{"jsonrpc":"2.0","method":"led.muteall","id":1}')
text(BP, 'list_leds', {}, '{"jsonrpc":"2.0","method":"led.list","id":1}')
text(BP, 'set_key_led', {'key': 'Menu', 'state': 'Blue'}, '{"jsonrpc":"2.0","method":"ui.leds.setstate","params":{"key":"Menu","state":"Blue"},"id":1}')
text(BP, 'toggle_menu', {}, '{"jsonrpc":"2.0","method":"ui.togglemenu","id":1}')
text(BP, 'toggle_osd', {}, '{"jsonrpc":"2.0","method":"ui.toggleosd","id":1}')
text(BP, 'toggle_lens_menu', {}, '{"jsonrpc":"2.0","method":"ui.togglelensmenu","id":1}')
text(BP, 'toggle_pattern_menu', {}, '{"jsonrpc":"2.0","method":"ui.togglepatternmenu","id":1}')
text(BP, 'toggle_source_menu', {}, '{"jsonrpc":"2.0","method":"ui.togglesourcemenu","id":1}')
text(BP, 'get_ui_setting', {'key': 'theme'}, '{"jsonrpc":"2.0","method":"ui.settings.get","params":{"key":"theme"},"id":1}')
text(BP, 'set_ui_setting', {'key': 'theme', 'value': 'dark'}, '{"jsonrpc":"2.0","method":"ui.settings.set","params":{"key":"theme","value":"dark"},"id":1}')
text(BP, 'remove_ui_setting', {'key': 'theme'}, '{"jsonrpc":"2.0","method":"ui.settings.remove","params":{"key":"theme"},"id":1}')
text(BP, 'list_ui_settings', {}, '{"jsonrpc":"2.0","method":"ui.settings.list","id":1}')
text(BP, 'list_ui_setting_keys', {}, '{"jsonrpc":"2.0","method":"ui.settings.keys","id":1}')
text(BP, 'pulse_gpo', {'output': 'trigger1'}, '{"jsonrpc":"2.0","method":"gpio.gpo.trigger1.pulse","id":1}')
text(BP, 'list_gpo_modes', {'output': 'trigger2'}, '{"jsonrpc":"2.0","method":"gpio.gpo.trigger2.listavailablemodes","id":1}')
text(BP, 'list_gpi', {}, '{"jsonrpc":"2.0","method":"gpio.listinputs","id":1}')
text(BP, 'list_gpo', {}, '{"jsonrpc":"2.0","method":"gpio.listoutputs","id":1}')
text(BP, 'list_dmx_channels', {}, '{"jsonrpc":"2.0","method":"dmx.listchannels","id":1}')
text(BP, 'list_dmx_modes', {}, '{"jsonrpc":"2.0","method":"dmx.listmodes","id":1}')
text(BP, 'list_network_devices', {}, '{"jsonrpc":"2.0","method":"network.list","id":1}')
text(BP, 'wifi_scan', {}, '{"jsonrpc":"2.0","method":"network.device.wifi.requestscan","id":1}')
text(BP, 'wifi_access_points', {}, '{"jsonrpc":"2.0","method":"network.device.wifi.getaccesspoints","id":1}')
text(BP, 'wifi_security', {'ssid': 'Venue'}, '{"jsonrpc":"2.0","method":"network.device.wifi.getsecurity","params":{"ssid":"Venue"},"id":1}')
text(BP, 'wifi_connect', {'ssid': 'Venue', 'security': 'WPA2', 'username': '', 'password': 'secret1'}, '{"jsonrpc":"2.0","method":"network.device.wifi.connect","params":{"ssid":"Venue","security":"WPA2","username":"","password":"secret1"},"id":1}')
text(BP, 'detect_peripherals', {}, '{"jsonrpc":"2.0","method":"peripheral.autodetect","id":1}')
text(BP, 'list_peripherals', {}, '{"jsonrpc":"2.0","method":"peripheral.list","id":1}')
text(BP, 'get_ranging_distance', {}, '{"jsonrpc":"2.0","method":"peripheral.ranging.getdistance","id":1}')
text(BP, 'create_scheduled_action', {'name': 'Morning'}, '{"jsonrpc":"2.0","method":"scheduler.create","params":{"name":"Morning"},"id":1}')
text(BP, 'list_ir_sensors', {}, '{"jsonrpc":"2.0","method":"remotecontrol.listsensors","id":1}')
V.append({'spec': BP, 'command': 'download_warp_grid', 'input': {}, 'expect_request': {'method': 'GET', 'port': 80, 'target': '/api/image/processing/warp/file/transfer'},
          'http_reply': {'status': 200, 'body': '<warp/>'}, 'expect_result': {'ok': {'kind': 'value', 'value': '<warp/>'}}})
V.append({'spec': BP, 'command': 'download_warp_file', 'input': {'filename': 'warpgrid.xml'}, 'expect_request': {'method': 'GET', 'port': 80, 'target': '/api/image/processing/warp/file/transfer/warpgrid.xml'}})

# Telemetry: pushes and replies as the catalogs show them.
telemetry(BP, 'property-changed', inbound='{"jsonrpc":"2.0","method":"property.changed","params":{"property":[{"illumination.sources.laser.power":40}]}}',
          expect_state={'illumination': {'sources': {'laser': {'power': 40.0}}}})
telemetry(BP, 'property-changed-several', inbound='{"jsonrpc":"2.0","method":"property.changed","params":{"property":[{"optics.shutter.position":"Closed"},{"image.connector.hdmi.detectedsignal":{"active":true,"name":"1920x1080 @ 60Hz"}},{"ui.osd":false}]}}',
          expect_state={'optics': {'shutter': {'position': 'Closed'}}, 'image': {'connector': {'hdmi': {'detectedsignal': '{"active":true,"name":"1920x1080 @ 60Hz"}'}}}, 'ui': {'osd': False}})
_GET = '{"jsonrpc":"2.0","method":"property.get","params":{"property":"%s"},"id":1}'
telemetry(BP, 'get-string', request=_GET % 'system.state', inbound='{"jsonrpc":"2.0","result":"on","id":1}',
          expect_state={'system': {'state': 'on'}})
telemetry(BP, 'get-number', request=_GET % 'image.brightness', inbound='{"jsonrpc":"2.0","id":7,"result":0.15}',
          expect_state={'image': {'brightness': 0.15}})
telemetry(BP, 'get-bool', request=_GET % 'optics.lenspresent', inbound='{"jsonrpc": "2.0", "result": true, "id": 1}',
          expect_state={'optics': {'lenspresent': True}})
telemetry(BP, 'get-object', request=_GET % 'image.window.main.position', inbound='{"jsonrpc":"2.0","result":{"x":0,"y":0},"id":1}',
          expect_state={'image': {'window': {'main': {'position': '{"x":0,"y":0}'}}}})
telemetry(BP, 'connector-list', request='{"jsonrpc":"2.0","method":"image.connector.list","id":1}',
          inbound='{"jsonrpc":"2.0","id":1,"result":["HDMI","DisplayPort 1"]}',
          expect_state={'image': {'connectors': '["HDMI","DisplayPort 1"]'}})
telemetry(BP, 'error-notifications', request='{"jsonrpc":"2.0","method":"notification.error.list","id":1}',
          inbound='{"jsonrpc":"2.0","id":1,"result":[]}',
          expect_state={'notification': {'error': {'active': '[]'}}})
telemetry(BP, 'temperatures', inbound='{"jsonrpc":"2.0","id":18,"result":{"environment.temperature.inlet":25.5,"environment.temperature.outlet":29.4}}',
          expect_state={'environment': {'sensors': {'environment.temperature.inlet': 25.5, 'environment.temperature.outlet': 29.4}}})
telemetry(BP, 'counters', inbound='{"jsonrpc":"2.0","id":1,"result":[{"name":"laser runtime","value":1234,"unit":"hours"},{"name":"uptime","value":56,"unit":"minutes"}]}',
          expect_state={'statistics': {'counters': {'laser runtime': {'value': 1234, 'unit': 'hours'}, 'uptime': {'value': 56, 'unit': 'minutes'}}}})
telemetry(BP, 'profile-activated', model='i600',
          inbound='{"jsonrpc":"2.0","method":"signal.callback","params":{"signal":[{"profile.profileactivated":{"name":"Show"}}]}}',
          expect_state={'signals': {'last': {'name': 'profile.profileactivated', 'arguments': '{"name":"Show"}'}}},
          expect_then_send=[_GET % 'profile.profiles', _GET % 'profile.presetassignments'])
telemetry(BP, 'notification-emitted', model='udx',
          inbound='{"jsonrpc":"2.0","method":"signal.callback","params":{"signal":[{"notification.emitted":{"id":"42"}}]}}',
          expect_state={'signals': {'last': {'name': 'notification.emitted', 'arguments': '{"id":"42"}'}}},
          expect_then_send=[_GET % 'notification.count', '{"jsonrpc":"2.0","method":"notification.list","id":1}'])
# The same JSON-RPC API on the RS232 port (link serial).
text(BP, 'get_property', {'property': 'image.brightness'}, '{"jsonrpc":"2.0","method":"property.get","params":{"property":"image.brightness"},"id":1}', device_reply='{"jsonrpc":"2.0","result":0.15,"id":1}', expect_result={'ok': {'kind': 'value', 'value': 0.15}}, settings={"link": "serial"}, file="get_property-serial")
# File endpoints: downloads saved to the host file named (never written by
# a vector), uploads streamed as the multipart field file.
V.append({'spec': BP, 'command': 'download_bio_log', 'input': {'to': {'file': 'download_bio_log.dat'}}, 'expect_request': {'method': 'GET', 'port': 80, 'target': '/api/bio/logger/transfer'}})
V.append({'spec': BP, 'command': 'download_notification_log', 'input': {'to': {'file': 'download_notification_log.dat'}}, 'expect_request': {'method': 'GET', 'port': 80, 'target': '/api/notification/logger/transfer'}})
V.append({'spec': BP, 'command': 'download_edid', 'input': {'to': {'file': 'download_edid.dat'}}, 'expect_request': {'method': 'GET', 'port': 80, 'target': '/api/image/connector/edid/transfer'}})
V.append({'spec': BP, 'command': 'download_black_level_file', 'input': {'to': {'file': 'download_black_level_file.dat'}}, 'expect_request': {'method': 'GET', 'port': 80, 'target': '/api/image/processing/blacklevel/file/transfer'}})
V.append({'spec': BP, 'command': 'download_blend_file', 'input': {'to': {'file': 'download_blend_file.dat'}}, 'expect_request': {'method': 'GET', 'port': 80, 'target': '/api/image/processing/blend/file/transfer'}})
V.append({'spec': BP, 'command': 'download_warp_grid_file', 'input': {'to': {'file': 'download_warp_grid_file.dat'}}, 'expect_request': {'method': 'GET', 'port': 80, 'target': '/api/image/processing/warp/file/transfer'}})
V.append({'spec': BP, 'command': 'download_test_pattern', 'input': {'to': {'file': 'download_test_pattern.dat'}}, 'expect_request': {'method': 'GET', 'port': 80, 'target': '/api/image/testpattern/file/transfer'}})
V.append({'spec': BP, 'command': 'download_license_registration', 'input': {'to': {'file': 'download_license_registration.dat'}}, 'expect_request': {'method': 'GET', 'port': 80, 'target': '/api/system/license/file/registration'}})
V.append({'spec': BP, 'command': 'capture_lcd', 'input': {'to': {'file': 'capture_lcd.dat'}}, 'expect_request': {'method': 'GET', 'port': 80, 'target': '/api/ui/capture/lcd'}})
V.append({'spec': BP, 'command': 'capture_osd', 'input': {'to': {'file': 'capture_osd.dat'}}, 'expect_request': {'method': 'GET', 'port': 80, 'target': '/api/ui/capture/osd'}})
V.append({'spec': BP, 'command': 'download_source_preview', 'input': {'to': {'file': 'download_source_preview.dat'}}, 'model': 'udx', 'expect_request': {'method': 'GET', 'port': 80, 'target': '/api/image/source/preview/transfer'}})
V.append({'spec': BP, 'command': 'download_source_image', 'input': {'to': {'file': 'download_source_image.dat'}}, 'model': 'udx', 'expect_request': {'method': 'GET', 'port': 80, 'target': '/api/image/source/transfer'}})
V.append({'spec': BP, 'command': 'download_camera_image', 'input': {'to': {'file': 'download_camera_image.dat'}}, 'model': 'udx', 'expect_request': {'method': 'GET', 'port': 80, 'target': '/api/peripheral/camera/transfer'}})


def _bp_form(data, filename):
    """multipart/form-data as the core writes it: one part, the field file,
    the boundary derived from the parts (SPEC.md §4, "Binary request bodies")."""
    import hashlib
    digest = hashlib.sha256(b"file\0" + data + b"\0").hexdigest()
    boundary = "----form-" + digest[:16]
    body = (f"--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n"
            "Content-Type: application/octet-stream\r\n\r\n").encode() + data + f"\r\n--{boundary}--\r\n".encode()
    return body.hex(), f"multipart/form-data; boundary={boundary}"


_BPB, _BPC = _bp_form(b"ABC", "x.dat")
V.append({'spec': BP, 'command': 'upload_edid', 'input': {'file': 'QUJD', 'filename': 'x.dat'}, 'expect_request': {'method': 'POST', 'port': 80, 'target': '/api/image/connector/edid/transfer', 'body_hex': _BPB, 'content_type': _BPC}})
V.append({'spec': BP, 'command': 'upload_black_level_file', 'input': {'file': 'QUJD', 'filename': 'x.dat'}, 'expect_request': {'method': 'POST', 'port': 80, 'target': '/api/image/processing/blacklevel/file/transfer', 'body_hex': _BPB, 'content_type': _BPC}})
V.append({'spec': BP, 'command': 'upload_blend_file', 'input': {'file': 'QUJD', 'filename': 'x.dat'}, 'expect_request': {'method': 'POST', 'port': 80, 'target': '/api/image/processing/blend/file/transfer', 'body_hex': _BPB, 'content_type': _BPC}})
V.append({'spec': BP, 'command': 'upload_warp_file', 'input': {'file': 'QUJD', 'filename': 'x.dat'}, 'expect_request': {'method': 'POST', 'port': 80, 'target': '/api/image/processing/warp/file/transfer', 'body_hex': _BPB, 'content_type': _BPC}})
V.append({'spec': BP, 'command': 'upload_test_pattern', 'input': {'file': 'QUJD', 'filename': 'x.dat'}, 'expect_request': {'method': 'POST', 'port': 80, 'target': '/api/image/testpattern/file/transfer', 'body_hex': _BPB, 'content_type': _BPC}})
V.append({'spec': BP, 'command': 'upload_splash_screen', 'input': {'file': 'QUJD', 'filename': 'x.dat'}, 'expect_request': {'method': 'POST', 'port': 80, 'target': '/api/ui/splashscreen/file', 'body_hex': _BPB, 'content_type': _BPC}})
telemetry(BP, 'internal-test-patterns', request='{"jsonrpc":"2.0","method":"image.testpattern.listinternal","id":1}', model='udx',
          inbound='{"jsonrpc":"2.0","result":[{"id":"crosshatch","name":"Crosshatch"}],"id":1}',
          expect_state={'image': {'testpattern': {'internal_patterns': '[{"id":"crosshatch","name":"Crosshatch"}]'}}})
telemetry(BP, 'nosignal-patterns', request='{"jsonrpc":"2.0","method":"image.testpattern.nosignal.list","id":1}', model='i600',
          inbound='{"jsonrpc":"2.0","result":["black","logo"],"id":1}',
          expect_state={'image': {'testpattern': {'nosignal': {'patterns': '["black","logo"]'}}}})
